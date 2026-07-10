use std::collections::HashMap;
use std::io::Read;

use thiserror::Error;

const MAX_REQUEST_HEADER_BYTES: usize = 64 * 1024;
const MAX_PART_HEADER_BYTES: usize = 16 * 1024;
pub const MAX_REASON_CHARS: usize = 2_000;
pub const MAX_EVIDENCE_FILES: usize = 3;
pub const MAX_EVIDENCE_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_REQUEST_BODY_BYTES: usize = MAX_EVIDENCE_FILES * MAX_EVIDENCE_BYTES + 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceUpload {
    pub extension: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackSubmission {
    pub reason: String,
    pub evidence: Vec<EvidenceUpload>,
}

#[derive(Debug, Error)]
pub enum UploadError {
    #[error("request io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("request headers exceed 64 KB")]
    HeadersTooLarge,
    #[error("malformed HTTP request: {0}")]
    MalformedRequest(String),
    #[error("request body exceeds {MAX_REQUEST_BODY_BYTES} bytes")]
    BodyTooLarge,
    #[error("truncated request body")]
    TruncatedBody,
    #[error("request changes requires multipart/form-data with a multipart boundary")]
    MissingMultipartBoundary,
    #[error("malformed multipart body: {0}")]
    MalformedMultipart(String),
    #[error("malformed multipart header")]
    MalformedMultipartHeader,
    #[error("request changes requires exactly one reason field")]
    InvalidReasonCount,
    #[error("reason must be 1-2000 characters")]
    InvalidReasonLength,
    #[error("request changes accepts at most 3 evidence images")]
    TooManyEvidenceFiles,
    #[error("evidence image exceeds 5 MB")]
    EvidenceTooLarge,
    #[error("unsupported image signature; use PNG, JPEG, or WebP")]
    UnsupportedImageSignature,
    #[error("unknown multipart field: {0}")]
    UnknownMultipartField(String),
}

pub fn read_http_request(reader: &mut impl Read) -> Result<HttpRequest, UploadError> {
    let mut bytes = Vec::new();
    let header_end = loop {
        if let Some(index) = find_bytes(&bytes, b"\r\n\r\n") {
            let header_end = index + 4;
            if header_end > MAX_REQUEST_HEADER_BYTES {
                return Err(UploadError::HeadersTooLarge);
            }
            break header_end;
        }
        if bytes.len() >= MAX_REQUEST_HEADER_BYTES {
            return Err(UploadError::HeadersTooLarge);
        }

        let remaining = MAX_REQUEST_HEADER_BYTES - bytes.len();
        let mut chunk = [0_u8; 4096];
        let read_limit = remaining.min(chunk.len());
        let read = reader.read(&mut chunk[..read_limit])?;
        if read == 0 {
            return Err(UploadError::MalformedRequest(
                "missing header terminator".to_owned(),
            ));
        }
        bytes.extend_from_slice(&chunk[..read]);
    };

    let (_, _, headers) = parse_request_head(&bytes[..header_end - 4])?;
    let content_length = content_length(&headers)?.unwrap_or(0);
    if content_length > MAX_REQUEST_BODY_BYTES {
        return Err(UploadError::BodyTooLarge);
    }

    let total_length = header_end + content_length;
    while bytes.len() < total_length {
        let mut chunk = [0_u8; 8192];
        let remaining = total_length - bytes.len();
        let read_limit = remaining.min(chunk.len());
        let read = reader.read(&mut chunk[..read_limit])?;
        if read == 0 {
            return Err(UploadError::TruncatedBody);
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
    bytes.truncate(total_length);
    parse_http_request(&bytes)
}

pub fn parse_http_request(bytes: &[u8]) -> Result<HttpRequest, UploadError> {
    let header_index = find_bytes(bytes, b"\r\n\r\n").ok_or_else(|| {
        if bytes.len() > MAX_REQUEST_HEADER_BYTES {
            UploadError::HeadersTooLarge
        } else {
            UploadError::MalformedRequest("missing header terminator".to_owned())
        }
    })?;
    let header_end = header_index + 4;
    if header_end > MAX_REQUEST_HEADER_BYTES {
        return Err(UploadError::HeadersTooLarge);
    }

    let (method, path, headers) = parse_request_head(&bytes[..header_index])?;
    let available_body = &bytes[header_end..];
    let declared_length = content_length(&headers)?;
    let body_length = declared_length.unwrap_or(available_body.len());
    if body_length > MAX_REQUEST_BODY_BYTES {
        return Err(UploadError::BodyTooLarge);
    }
    if available_body.len() < body_length {
        return Err(UploadError::TruncatedBody);
    }

    Ok(HttpRequest {
        method,
        path,
        headers,
        body: available_body[..body_length].to_vec(),
    })
}

pub fn parse_request_changes(request: &HttpRequest) -> Result<FeedbackSubmission, UploadError> {
    let boundary = multipart_boundary(request)?;
    let parts = multipart_parts(&request.body, boundary.as_bytes())?;
    let mut reason = None;
    let mut evidence = Vec::new();

    for part in parts {
        let disposition = part
            .headers
            .get("content-disposition")
            .ok_or(UploadError::MalformedMultipartHeader)?;
        let parameters = disposition_parameters(disposition)?;
        let field_name = parameters
            .get("name")
            .ok_or(UploadError::MalformedMultipartHeader)?;
        match field_name.as_str() {
            "reason" => {
                if reason.is_some() || parameters.contains_key("filename") {
                    return Err(UploadError::InvalidReasonCount);
                }
                let value = std::str::from_utf8(part.body).map_err(|_| {
                    UploadError::MalformedMultipart("reason is not valid UTF-8".to_owned())
                })?;
                reason = Some(value.trim().to_owned());
            }
            "evidence" => {
                if !parameters.contains_key("filename") {
                    return Err(UploadError::MalformedMultipartHeader);
                }
                if evidence.len() >= MAX_EVIDENCE_FILES {
                    return Err(UploadError::TooManyEvidenceFiles);
                }
                if part.body.len() > MAX_EVIDENCE_BYTES {
                    return Err(UploadError::EvidenceTooLarge);
                }
                let (extension, content_type) = image_type(part.body)?;
                evidence.push(EvidenceUpload {
                    extension: extension.to_owned(),
                    content_type: content_type.to_owned(),
                    bytes: part.body.to_vec(),
                });
            }
            unknown => return Err(UploadError::UnknownMultipartField(unknown.to_owned())),
        }
    }

    let reason = reason.ok_or(UploadError::InvalidReasonCount)?;
    let reason_chars = reason.chars().count();
    if reason_chars == 0 || reason_chars > MAX_REASON_CHARS {
        return Err(UploadError::InvalidReasonLength);
    }
    Ok(FeedbackSubmission { reason, evidence })
}

fn parse_request_head(
    bytes: &[u8],
) -> Result<(String, String, HashMap<String, String>), UploadError> {
    let head = std::str::from_utf8(bytes)
        .map_err(|_| UploadError::MalformedRequest("headers are not valid UTF-8".to_owned()))?;
    let mut lines = head.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| UploadError::MalformedRequest("missing request line".to_owned()))?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next();
    let path = request_parts.next();
    let version = request_parts.next();
    if method.is_none()
        || path.is_none()
        || version.is_none()
        || request_parts.next().is_some()
        || !version.is_some_and(|value| value.starts_with("HTTP/"))
    {
        return Err(UploadError::MalformedRequest(
            "invalid request line".to_owned(),
        ));
    }

    let mut headers = HashMap::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| UploadError::MalformedRequest("malformed header".to_owned()))?;
        let name = name.trim().to_ascii_lowercase();
        if name.is_empty() || headers.contains_key(&name) {
            return Err(UploadError::MalformedRequest(
                "duplicate or empty header".to_owned(),
            ));
        }
        headers.insert(name, value.trim().to_owned());
    }

    Ok((
        method.unwrap_or_default().to_owned(),
        path.unwrap_or_default().to_owned(),
        headers,
    ))
}

fn content_length(headers: &HashMap<String, String>) -> Result<Option<usize>, UploadError> {
    if headers.contains_key("transfer-encoding") {
        return Err(UploadError::MalformedRequest(
            "transfer encoding is not supported".to_owned(),
        ));
    }
    headers
        .get("content-length")
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|_| UploadError::MalformedRequest("invalid Content-Length".to_owned()))
        })
        .transpose()
}

fn multipart_boundary(request: &HttpRequest) -> Result<String, UploadError> {
    let content_type = request
        .headers
        .get("content-type")
        .ok_or(UploadError::MissingMultipartBoundary)?;
    let mut segments = content_type.split(';');
    if !segments
        .next()
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("multipart/form-data"))
    {
        return Err(UploadError::MissingMultipartBoundary);
    }
    let boundary = segments.find_map(|segment| {
        let (name, value) = segment.trim().split_once('=')?;
        name.trim()
            .eq_ignore_ascii_case("boundary")
            .then(|| unquote(value.trim()).to_owned())
    });
    match boundary {
        Some(value)
            if !value.is_empty()
                && value.len() <= 70
                && !value.bytes().any(|byte| byte == b'\r' || byte == b'\n') =>
        {
            Ok(value)
        }
        _ => Err(UploadError::MissingMultipartBoundary),
    }
}

struct MultipartPart<'a> {
    headers: HashMap<String, String>,
    body: &'a [u8],
}

fn multipart_parts<'a>(
    body: &'a [u8],
    boundary: &[u8],
) -> Result<Vec<MultipartPart<'a>>, UploadError> {
    let mut delimiter = Vec::with_capacity(boundary.len() + 2);
    delimiter.extend_from_slice(b"--");
    delimiter.extend_from_slice(boundary);
    if !body.starts_with(&delimiter) {
        return Err(UploadError::MalformedMultipart(
            "missing opening boundary".to_owned(),
        ));
    }

    let mut next_marker = Vec::with_capacity(delimiter.len() + 2);
    next_marker.extend_from_slice(b"\r\n");
    next_marker.extend_from_slice(&delimiter);
    let mut cursor = 0;
    let mut parts = Vec::new();
    loop {
        cursor += delimiter.len();
        if body.get(cursor..cursor + 2) == Some(b"--") {
            cursor += 2;
            if body
                .get(cursor..)
                .is_some_and(|tail| tail.is_empty() || tail == b"\r\n")
            {
                return Ok(parts);
            }
            return Err(UploadError::MalformedMultipart(
                "unexpected bytes after closing boundary".to_owned(),
            ));
        }
        if body.get(cursor..cursor + 2) != Some(b"\r\n") {
            return Err(UploadError::MalformedMultipart(
                "boundary is not followed by headers".to_owned(),
            ));
        }
        cursor += 2;

        let header_relative = find_bytes(&body[cursor..], b"\r\n\r\n")
            .ok_or(UploadError::MalformedMultipartHeader)?;
        if header_relative > MAX_PART_HEADER_BYTES {
            return Err(UploadError::MalformedMultipartHeader);
        }
        let header_end = cursor + header_relative;
        let headers = parse_part_headers(&body[cursor..header_end])?;
        let content_start = header_end + 4;
        let content_relative =
            find_bytes(&body[content_start..], &next_marker).ok_or_else(|| {
                UploadError::MalformedMultipart("missing closing boundary".to_owned())
            })?;
        let content_end = content_start + content_relative;
        parts.push(MultipartPart {
            headers,
            body: &body[content_start..content_end],
        });
        cursor = content_end + 2;
    }
}

fn parse_part_headers(bytes: &[u8]) -> Result<HashMap<String, String>, UploadError> {
    let text = std::str::from_utf8(bytes).map_err(|_| UploadError::MalformedMultipartHeader)?;
    let mut headers = HashMap::new();
    for line in text.split("\r\n") {
        let (name, value) = line
            .split_once(':')
            .ok_or(UploadError::MalformedMultipartHeader)?;
        let name = name.trim().to_ascii_lowercase();
        if name.is_empty() || headers.insert(name, value.trim().to_owned()).is_some() {
            return Err(UploadError::MalformedMultipartHeader);
        }
    }
    Ok(headers)
}

fn disposition_parameters(value: &str) -> Result<HashMap<String, String>, UploadError> {
    let mut segments = value.split(';');
    if !segments
        .next()
        .is_some_and(|segment| segment.trim().eq_ignore_ascii_case("form-data"))
    {
        return Err(UploadError::MalformedMultipartHeader);
    }
    let mut parameters = HashMap::new();
    for segment in segments {
        let (name, value) = segment
            .trim()
            .split_once('=')
            .ok_or(UploadError::MalformedMultipartHeader)?;
        let name = name.trim().to_ascii_lowercase();
        let value = unquote(value.trim());
        if name.is_empty()
            || value.is_empty()
            || parameters.insert(name, value.to_owned()).is_some()
        {
            return Err(UploadError::MalformedMultipartHeader);
        }
    }
    Ok(parameters)
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
}

fn image_type(bytes: &[u8]) -> Result<(&'static str, &'static str), UploadError> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        return Ok(("png", "image/png"));
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Ok(("jpg", "image/jpeg"));
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Ok(("webp", "image/webp"));
    }
    Err(UploadError::UnsupportedImageSignature)
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn request_bytes(method: &str, path: &str, content_type: &str, body: &[u8]) -> Vec<u8> {
        let mut request = format!(
            "{method} {path} HTTP/1.1\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .into_bytes();
        request.extend_from_slice(body);
        request
    }

    fn multipart_body(
        boundary: &str,
        reasons: &[&str],
        evidence: &[(&str, &str, &[u8])],
        extra_fields: &[(&str, &str)],
    ) -> Vec<u8> {
        let mut body = Vec::new();
        for reason in reasons {
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            body.extend_from_slice(b"Content-Disposition: form-data; name=\"reason\"\r\n\r\n");
            body.extend_from_slice(reason.as_bytes());
            body.extend_from_slice(b"\r\n");
        }
        for (name, content_type, bytes) in evidence {
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            body.extend_from_slice(
                format!(
                    "Content-Disposition: form-data; name=\"evidence\"; filename=\"{name}\"\r\nContent-Type: {content_type}\r\n\r\n"
                )
                .as_bytes(),
            );
            body.extend_from_slice(bytes);
            body.extend_from_slice(b"\r\n");
        }
        for (name, value) in extra_fields {
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            body.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
            );
            body.extend_from_slice(value.as_bytes());
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        body
    }

    fn multipart_request(
        boundary: &str,
        reasons: &[&str],
        evidence: &[(&str, &str, &[u8])],
    ) -> HttpRequest {
        let body = multipart_body(boundary, reasons, evidence, &[]);
        parse_http_request(&request_bytes(
            "POST",
            "/upload",
            &format!("multipart/form-data; boundary={boundary}"),
            &body,
        ))
        .unwrap()
    }

    #[test]
    fn request_changes_reads_body_larger_than_legacy_buffer() {
        let body = vec![b'x'; 12_000];
        let request = request_bytes("POST", "/upload", "application/octet-stream", &body);
        let parsed = read_http_request(&mut Cursor::new(request)).unwrap();
        assert_eq!(parsed.body, body);
    }

    #[test]
    fn request_changes_refuses_content_length_above_ceiling() {
        let request = format!(
            "POST /upload HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            MAX_REQUEST_BODY_BYTES + 1
        );
        let error = read_http_request(&mut Cursor::new(request.into_bytes())).unwrap_err();
        assert!(error.to_string().contains("request body exceeds"));
    }

    #[test]
    fn request_changes_parses_reason_and_valid_png() {
        let png = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3];
        let request = multipart_request(
            "boundary",
            &["  Fix mobile spacing  "],
            &[("proof.png", "image/png", &png)],
        );
        let feedback = parse_request_changes(&request).unwrap();
        assert_eq!(feedback.reason, "Fix mobile spacing");
        assert_eq!(feedback.evidence[0].extension, "png");
        assert_eq!(feedback.evidence[0].content_type, "image/png");
        assert_eq!(feedback.evidence[0].bytes, png);
    }

    #[test]
    fn request_changes_accepts_jpeg_and_webp_signatures() {
        let jpeg = [0xff, 0xd8, 0xff, 0xe0, 1, 2, 3];
        let webp = [
            b'R', b'I', b'F', b'F', 4, 0, 0, 0, b'W', b'E', b'B', b'P', 1, 2,
        ];
        let request = multipart_request(
            "boundary",
            &["Fix it"],
            &[
                ("proof.jpg", "image/jpeg", &jpeg),
                ("proof.webp", "image/webp", &webp),
            ],
        );

        let feedback = parse_request_changes(&request).unwrap();

        assert_eq!(feedback.evidence[0].extension, "jpg");
        assert_eq!(feedback.evidence[0].content_type, "image/jpeg");
        assert_eq!(feedback.evidence[1].extension, "webp");
        assert_eq!(feedback.evidence[1].content_type, "image/webp");
    }

    #[test]
    fn request_changes_rejects_misleading_image_mime() {
        let request = multipart_request(
            "boundary",
            &["Fix it"],
            &[("proof.png", "image/png", b"not png")],
        );
        let error = parse_request_changes(&request).unwrap_err();
        assert!(error.to_string().contains("unsupported image signature"));
    }

    #[test]
    fn request_changes_rejects_missing_or_oversized_reason() {
        let missing = multipart_request("boundary", &["   "], &[]);
        assert!(parse_request_changes(&missing)
            .unwrap_err()
            .to_string()
            .contains("reason must be 1-2000 characters"));

        let oversized = "x".repeat(MAX_REASON_CHARS + 1);
        let request = multipart_request("boundary", &[&oversized], &[]);
        assert!(parse_request_changes(&request)
            .unwrap_err()
            .to_string()
            .contains("reason must be 1-2000 characters"));
    }

    #[test]
    fn request_changes_rejects_duplicate_reason_fields() {
        let request = multipart_request("boundary", &["First", "Second"], &[]);
        assert!(parse_request_changes(&request)
            .unwrap_err()
            .to_string()
            .contains("exactly one reason"));
    }

    #[test]
    fn request_changes_rejects_more_than_three_images() {
        let png = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        let request = multipart_request(
            "boundary",
            &["Fix it"],
            &[
                ("1.png", "image/png", &png),
                ("2.png", "image/png", &png),
                ("3.png", "image/png", &png),
                ("4.png", "image/png", &png),
            ],
        );
        assert!(parse_request_changes(&request)
            .unwrap_err()
            .to_string()
            .contains("at most 3 evidence images"));
    }

    #[test]
    fn request_changes_rejects_image_above_five_megabytes() {
        let mut png = vec![0_u8; MAX_EVIDENCE_BYTES + 1];
        png[..8].copy_from_slice(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        let request =
            multipart_request("boundary", &["Fix it"], &[("large.png", "image/png", &png)]);
        assert!(parse_request_changes(&request)
            .unwrap_err()
            .to_string()
            .contains("evidence image exceeds 5 MB"));
    }

    #[test]
    fn request_changes_requires_multipart_boundary() {
        let request = parse_http_request(
            b"POST /upload HTTP/1.1\r\nContent-Type: multipart/form-data\r\nContent-Length: 0\r\n\r\n",
        )
        .unwrap();
        assert!(parse_request_changes(&request)
            .unwrap_err()
            .to_string()
            .contains("multipart boundary"));
    }

    #[test]
    fn request_changes_rejects_malformed_part_headers() {
        let body = b"--boundary\r\nBroken header\r\n\r\nvalue\r\n--boundary--\r\n";
        let request = parse_http_request(&request_bytes(
            "POST",
            "/upload",
            "multipart/form-data; boundary=boundary",
            body,
        ))
        .unwrap();
        assert!(parse_request_changes(&request)
            .unwrap_err()
            .to_string()
            .contains("malformed multipart header"));
    }

    #[test]
    fn request_changes_rejects_truncated_http_body() {
        let request = b"POST /upload HTTP/1.1\r\nContent-Length: 10\r\n\r\nshort";
        assert!(parse_http_request(request)
            .unwrap_err()
            .to_string()
            .contains("truncated request body"));
    }

    #[test]
    fn request_changes_rejects_unknown_multipart_fields() {
        let body = multipart_body("boundary", &["Fix it"], &[], &[("surprise", "value")]);
        let request = parse_http_request(&request_bytes(
            "POST",
            "/upload",
            "multipart/form-data; boundary=boundary",
            &body,
        ))
        .unwrap();
        assert!(parse_request_changes(&request)
            .unwrap_err()
            .to_string()
            .contains("unknown multipart field"));
    }
}
