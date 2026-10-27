//! Fuzz target: the multipart encoder on arbitrary names, values and payloads.
//!
//! Input is NUL separated: `name \0 value \0 filename \0 mime \0 payload`.
//!
//! Invariants:
//! 1. Encoding never panics, is deterministic, and closes with the trailer.
//! 2. There is exactly one `Content-Disposition` line and, for a file part,
//!    exactly one `Content-Type` line per part — a name, a file name or a MIME
//!    type cannot smuggle a second header line in.
//! 3. The payload arrives byte for byte: no escaping touches body content.

#![no_main]

use libfuzzer_sys::fuzz_target;
use telegram_api_client::{MultipartFile, MultipartForm};

/// Position of `needle` in `haystack`.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Three lines above (`Content-Disposition`, `Content-Type`) or two (a plain
/// field) are fine; more means a caller-supplied value added a line break.
fn assert_header_lines(header: &[u8], expected: usize) {
    let lines = header.split(|byte| *byte == b'\n').count();
    assert_eq!(
        lines,
        expected,
        "header block has {} lines, expected {expected}: {:?}",
        lines,
        String::from_utf8_lossy(header)
    );
    assert!(
        find(header, b"\r\r").is_none() && !header.ends_with(b"\r"),
        "malformed line break in header: {:?}",
        String::from_utf8_lossy(header)
    );
}

fuzz_target!(|data: &[u8]| {
    let mut chunks = data.splitn(5, |byte| *byte == 0);
    let name = String::from_utf8_lossy(chunks.next().unwrap_or_default()).into_owned();
    let value = String::from_utf8_lossy(chunks.next().unwrap_or_default()).into_owned();
    let filename = String::from_utf8_lossy(chunks.next().unwrap_or_default()).into_owned();
    let mime = String::from_utf8_lossy(chunks.next().unwrap_or_default()).into_owned();
    let payload = chunks.next().unwrap_or_default().to_vec();

    let form = MultipartForm::new()
        .field(&name, &value)
        .with_file(MultipartFile::new(&name, &filename, &mime, payload.clone()));

    let (boundary, body) = form.to_http_body();
    assert_eq!(
        body,
        form.encode(&boundary),
        "encoding is not deterministic"
    );

    let delimiter = format!("--{boundary}\r\n");
    assert!(body.starts_with(delimiter.as_bytes()), "missing first part");
    assert!(
        body.ends_with(format!("--{boundary}--\r\n").as_bytes()),
        "missing trailer"
    );

    // The payload is carried verbatim, so it can contain anything — including,
    // in principle, the boundary. Only count delimiters when it does not.
    let payload_holds_boundary = payload
        .windows(boundary.len())
        .any(|window| window == boundary.as_bytes());
    if !payload_holds_boundary {
        let count = body
            .windows(delimiter.len())
            .filter(|window| *window == delimiter.as_bytes())
            .count();
        assert_eq!(count, 2, "one delimiter per part");
    }

    let mut rest = body.as_slice();
    let mut part = 0_usize;
    while let Some(offset) = find(rest, delimiter.as_bytes()) {
        let after = &rest[offset + delimiter.len()..];
        let Some(end) = find(after, b"\r\n\r\n") else {
            break;
        };
        let expected = if part == 0 { 1 } else { 2 };
        assert_header_lines(&after[..end], expected);
        rest = &after[end + 4..];
        part += 1;
    }
    assert_eq!(part, 2, "expected a field part and a file part");
});
