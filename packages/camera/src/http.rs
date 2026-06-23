//! Minimal blocking HTTP/1.0 client, deliberately **iSH-safe**.
//!
//! It sets NO socket options (timeouts / non-blocking) — those are what made
//! `ureq` fail with EINVAL on iSH (iOS rejects `setsockopt` timeouts). Using
//! HTTP/1.0 with `Connection: close` lets us read the body to EOF without
//! parsing Content-Length or chunked encoding.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

pub(crate) const UA: &str = "UPnP/1.0 DLNADOC/1.50 Sony";

/// Split `http://host:port/path` into its parts (default port 80, default path /).
pub(crate) fn split_url(url: &str) -> Result<(String, u16, String), String> {
    let rest = url
        .strip_prefix("http://")
        .ok_or("only http:// is supported")?;
    let slash = rest.find('/').unwrap_or(rest.len());
    let authority = &rest[..slash];
    let path = if slash < rest.len() {
        &rest[slash..]
    } else {
        "/"
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse().map_err(|_| "bad port")?),
        None => (authority.to_string(), 80u16),
    };
    Ok((host, port, path.to_string()))
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Perform one blocking HTTP/1.0 request; returns (status, body, content_type).
pub(crate) fn http_request(
    method: &str,
    url: &str,
    extra_headers: &[(&str, &str)],
    body: Option<&[u8]>,
) -> Result<(u16, Vec<u8>, String), String> {
    let (host, port, path) = split_url(url)?;
    let mut stream = TcpStream::connect((host.as_str(), port))
        .map_err(|e| format!("connect {host}:{port}: {e}"))?;

    let mut req = format!("{method} {path} HTTP/1.0\r\nHost: {host}:{port}\r\n");
    for (k, v) in extra_headers {
        req.push_str(k);
        req.push_str(": ");
        req.push_str(v);
        req.push_str("\r\n");
    }
    if let Some(b) = body {
        req.push_str(&format!("Content-Length: {}\r\n", b.len()));
    }
    req.push_str("Connection: close\r\n\r\n");

    stream
        .write_all(req.as_bytes())
        .map_err(|e| e.to_string())?;
    if let Some(b) = body {
        stream.write_all(b).map_err(|e| e.to_string())?;
    }
    stream.flush().ok();

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).map_err(|e| e.to_string())?;

    let sep = find_subslice(&raw, b"\r\n\r\n").ok_or("malformed HTTP response")?;
    let head = String::from_utf8_lossy(&raw[..sep]).into_owned();
    let body_bytes = raw[sep + 4..].to_vec();

    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0u16);
    let mut content_type = String::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-type") {
                content_type = v.trim().to_string();
            }
        }
    }
    Ok((status, body_bytes, content_type))
}

/// GET a URL, requiring 200; returns (body, content_type) defaulting to JPEG.
pub(crate) fn http_get_bytes(url: &str) -> Result<(Vec<u8>, String), String> {
    let (status, body, ct) = http_request("GET", url, &[("User-Agent", UA)], None)?;
    if status != 200 {
        return Err(format!("{url}: HTTP {status}"));
    }
    Ok((
        body,
        if ct.is_empty() {
            "image/jpeg".to_string()
        } else {
            ct
        },
    ))
}

/// Issue a ContentDirectory `Browse` SOAP action (BrowseDirectChildren) for a
/// page `[start, start+count)`, retrying transient 503s.
pub(crate) fn soap_browse(
    control_url: &str,
    service_type: &str,
    object_id: &str,
    start: usize,
    count: usize,
) -> Result<Vec<u8>, String> {
    let body = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\
<s:Body><u:Browse xmlns:u=\"{st}\"><ObjectID>{oid}</ObjectID>\
<BrowseFlag>BrowseDirectChildren</BrowseFlag><Filter>*</Filter>\
<StartingIndex>{start}</StartingIndex><RequestedCount>{count}</RequestedCount>\
<SortCriteria></SortCriteria></u:Browse></s:Body></s:Envelope>",
        st = service_type,
        oid = object_id
    );
    let soapaction = format!("\"{service_type}#Browse\"");
    let headers = [
        ("User-Agent", UA),
        ("Content-Type", "text/xml; charset=\"utf-8\""),
        ("SOAPACTION", soapaction.as_str()),
    ];
    let mut last = String::new();
    for attempt in 0..3 {
        match http_request("POST", control_url, &headers, Some(body.as_bytes())) {
            Ok((200, b, _)) => return Ok(b),
            Ok((status, _, _)) => {
                last = format!("Browse {object_id}: HTTP {status}");
                std::thread::sleep(Duration::from_millis(400 * (attempt + 1)));
            }
            Err(e) => {
                last = e;
                std::thread::sleep(Duration::from_millis(400 * (attempt + 1)));
            }
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn split_url_parses_authority_and_path() {
        assert_eq!(
            split_url("http://10.0.0.1:64321/DmsDesc.xml").unwrap(),
            ("10.0.0.1".to_string(), 64321, "/DmsDesc.xml".to_string())
        );
        assert_eq!(
            split_url("http://host").unwrap(),
            ("host".to_string(), 80, "/".to_string())
        );
    }

    #[test]
    fn http_request_round_trips() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut buf = [0u8; 2048];
            let _ = s.read(&mut buf);
            s.write_all(b"HTTP/1.0 200 OK\r\nContent-Type: image/jpeg\r\n\r\nDATA")
                .unwrap();
        });
        let url = format!("http://127.0.0.1:{}/x", addr.port());
        let (status, body, ct) = http_request("GET", &url, &[("User-Agent", "t")], None).unwrap();
        assert_eq!(status, 200);
        assert_eq!(body, b"DATA");
        assert_eq!(ct, "image/jpeg");
        server.join().unwrap();
    }
}
