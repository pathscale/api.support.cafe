//! Outbound HTTPS, one request per connection, over a nagoya reactor.
//!
//! This replaces reqwest (Doppler) and tgbot's client (Telegram), which were
//! the last reasons tokio was in the graph. Inbound is plain HTTP behind Fly's
//! TLS; this is only the client side, for the two services that are HTTPS-only.
//!
//! Deliberately small: HTTP/1.1, `Connection: close`, a body framed by
//! `Content-Length` or `chunked`, and nothing else. Both callers send one
//! request at a time and neither is on a hot path, so connection reuse would be
//! complexity bought for nothing.
//!
//! # Resolving is separate, and synchronous
//!
//! [`Target::resolve`] is `getaddrinfo`, which blocks its thread. It must run
//! before the reactor is driven, or on a thread whose reactor serves nothing
//! else, for the reason endpoint-libs' `WsTarget::resolve` documents: a lookup
//! inside a running reactor stalls every socket on it.

use eyre::{Result, bail, eyre};
use nago_rustls::TlsSession;
use nago_rustls::rustls::ClientConnection;
use nago_rustls::rustls_pki_types::ServerName;
use nagoya::reactor::{Addr, Handle, connect_any, resolve};

/// Largest response this will buffer. Telegram and Doppler answers are a few
/// kilobytes; anything near this is a fault, not a payload.
const MAX_RESPONSE: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Target {
    host: String,
    addrs: Vec<Addr>,
}

impl Target {
    /// Resolve `host` on port 443. Blocks the calling thread for the lookup.
    pub fn resolve(host: &str) -> Result<Self> {
        let addrs =
            resolve(host, 443).map_err(|err| eyre!("DNS resolution failed for {host}: {err}"))?;
        if addrs.is_empty() {
            bail!("DNS resolution for {host} returned no addresses");
        }
        Ok(Self {
            host: host.to_string(),
            addrs,
        })
    }
}

#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

impl Response {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

pub struct Request<'a> {
    pub method: &'a str,
    /// Path and query, already percent-encoded.
    pub path: &'a str,
    pub headers: &'a [(&'a str, &'a str)],
    /// The body and its content type.
    pub body: Option<(&'a str, &'a [u8])>,
}

/// Send one request and read the whole response.
pub async fn send(target: &Target, handle: &Handle, request: Request<'_>) -> Result<Response> {
    let stream = connect_any(&target.addrs, handle)
        .await
        .map_err(|err| eyre!("TCP connect to {} failed: {err}", target.host))?;
    let name = ServerName::try_from(target.host.clone())
        .map_err(|_| eyre!("invalid TLS server name: {}", target.host))?;
    let session = ClientConnection::new(nago_rustls::default_client_config(), name)
        .map_err(|err| eyre!("TLS session setup failed: {err}"))?;
    let mut tls = TlsSession::client(stream, session);
    tls.handshake()
        .await
        .map_err(|err| eyre!("TLS handshake with {} failed: {err:?}", target.host))?;

    tls.write_all(&encode_request(&target.host, &request))
        .await
        .map_err(|err| eyre!("writing request to {} failed: {err:?}", target.host))?;

    let mut buffer = Vec::with_capacity(16 * 1024);
    let mut chunk = [0u8; 16 * 1024];
    loop {
        if let Some(response) = parse_response(&buffer, false)? {
            let _ = tls.close().await;
            return Ok(response);
        }
        let read = tls
            .read(&mut chunk)
            .await
            .map_err(|err| eyre!("reading response from {} failed: {err:?}", target.host))?;
        if read == 0 {
            return parse_response(&buffer, true)?
                .ok_or_else(|| eyre!("{} closed the connection mid-response", target.host));
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.len() > MAX_RESPONSE {
            bail!(
                "response from {} exceeded {MAX_RESPONSE} bytes",
                target.host
            );
        }
    }
}

fn encode_request(host: &str, request: &Request<'_>) -> Vec<u8> {
    let mut head = format!(
        "{} {} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nAccept-Encoding: identity\r\n",
        request.method, request.path
    );
    for (name, value) in request.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    let body: &[u8] = match request.body {
        Some((content_type, body)) => {
            head.push_str(&format!(
                "Content-Type: {content_type}\r\nContent-Length: {}\r\n",
                body.len()
            ));
            body
        }
        None => &[],
    };
    head.push_str("\r\n");
    let mut bytes = head.into_bytes();
    bytes.extend_from_slice(body);
    bytes
}

/// Parse a complete response out of `buffer`, or `None` if more is needed.
///
/// `at_eof` says the peer has closed: a body framed by neither length nor
/// chunking ends there, and one that is framed but short is an error.
fn parse_response(buffer: &[u8], at_eof: bool) -> Result<Option<Response>> {
    let Some(head_end) = find(buffer, b"\r\n\r\n") else {
        if at_eof {
            bail!("connection closed before the response headers ended");
        }
        return Ok(None);
    };
    let head = std::str::from_utf8(&buffer[..head_end])
        .map_err(|_| eyre!("response headers are not UTF-8"))?;
    let mut lines = head.split("\r\n");
    let status_line = lines.next().unwrap_or_default();
    let status = status_line
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| eyre!("malformed status line: {status_line}"))?;

    let mut content_length = None;
    let mut chunked = false;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            content_length = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| eyre!("malformed Content-Length: {value}"))?,
            );
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            chunked = value.to_ascii_lowercase().contains("chunked");
        }
    }

    let body = &buffer[head_end + 4..];
    let body = if chunked {
        match decode_chunked(body)? {
            Some(body) => body,
            None if at_eof => bail!("connection closed inside a chunked body"),
            None => return Ok(None),
        }
    } else if let Some(length) = content_length {
        if body.len() < length {
            if at_eof {
                bail!(
                    "connection closed {} bytes short of the body",
                    length - body.len()
                );
            }
            return Ok(None);
        }
        body[..length].to_vec()
    } else if at_eof {
        body.to_vec()
    } else {
        return Ok(None);
    };
    Ok(Some(Response { status, body }))
}

/// Decode a chunked body, or `None` if the terminating chunk has not arrived.
fn decode_chunked(mut input: &[u8]) -> Result<Option<Vec<u8>>> {
    let mut body = Vec::new();
    loop {
        let Some(line_end) = find(input, b"\r\n") else {
            return Ok(None);
        };
        let size_field = std::str::from_utf8(&input[..line_end])
            .map_err(|_| eyre!("chunk size is not UTF-8"))?;
        let size_hex = size_field.split(';').next().unwrap_or_default().trim();
        let size = usize::from_str_radix(size_hex, 16)
            .map_err(|_| eyre!("malformed chunk size: {size_field}"))?;
        input = &input[line_end + 2..];
        if size == 0 {
            // Trailers are not used by either peer; the terminator is enough.
            return Ok(Some(body));
        }
        if input.len() < size + 2 {
            return Ok(None);
        }
        body.extend_from_slice(&input[..size]);
        input = &input[size + 2..];
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_length_body_waits_for_every_byte() {
        let partial = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhel";
        assert!(parse_response(partial, false).unwrap().is_none());
        assert!(parse_response(partial, true).is_err());

        let full = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello";
        let response = parse_response(full, false).unwrap().unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"hello");
    }

    #[test]
    fn chunked_body_is_reassembled() {
        let raw = b"HTTP/1.1 502 Bad Gateway\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nwiki\r\n5;x=y\r\npedia\r\n0\r\n\r\n";
        let response = parse_response(raw, false).unwrap().unwrap();
        assert_eq!(response.status, 502);
        assert!(!response.is_success());
        assert_eq!(response.body, b"wikipedia");

        let unfinished = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nwiki\r\n";
        assert!(parse_response(unfinished, false).unwrap().is_none());
    }

    #[test]
    fn unframed_body_ends_at_eof() {
        let raw = b"HTTP/1.1 200 OK\r\n\r\nall of it";
        assert!(parse_response(raw, false).unwrap().is_none());
        assert_eq!(
            parse_response(raw, true).unwrap().unwrap().body,
            b"all of it"
        );
    }

    #[test]
    fn request_carries_host_length_and_close() {
        let bytes = encode_request(
            "api.example.com",
            &Request {
                method: "POST",
                path: "/x?y=1",
                headers: &[("Authorization", "Bearer t")],
                body: Some(("application/json", b"{}")),
            },
        );
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.starts_with("POST /x?y=1 HTTP/1.1\r\nHost: api.example.com\r\n"));
        assert!(text.contains("Connection: close\r\n"));
        assert!(text.contains("Authorization: Bearer t\r\n"));
        assert!(text.ends_with("Content-Length: 2\r\n\r\n{}"));
    }
}
