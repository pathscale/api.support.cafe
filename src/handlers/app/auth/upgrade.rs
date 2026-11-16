use std::io::{Cursor, Read};
use std::net::SocketAddr;

use endpoint_libs::libs::ws::{
    BoxedStream, NagoWssUpgrader, RawStream, StreamError, UpgradeEvent, WsServerConfig, WsUpgrader,
};
use futures::future::LocalBoxFuture;
use nago_wss::proto::handshake::{DEFAULT_MAX_HEAD, Parsed, head_end, parse_request};

const METHOD: &str = "0appconnect";
const PROTOCOL_HEADER: &str = "sec-websocket-protocol";

fn numbered_ascii(token: &str, index: char, length: usize, valid: fn(&u8) -> bool) -> bool {
    token
        .strip_prefix(index)
        .is_some_and(|value| value.len() == length && value.as_bytes().iter().all(valid))
}

fn looks_like_proof(token: &str) -> bool {
    let bytes = token.as_bytes();
    let proof = if bytes.len() == 65 && bytes[0].is_ascii_digit() {
        &bytes[1..]
    } else {
        bytes
    };
    proof.len() == 64 && proof.iter().all(u8::is_ascii_hexdigit)
}

fn visitor_offer_is_safe(request: &nago_wss::proto::handshake::Request) -> bool {
    // A proof offered first must never become the selected subprotocol, even
    // when the sender dropped or misspelled the method.
    if request
        .protocols
        .first()
        .is_some_and(|first| looks_like_proof(first))
    {
        return false;
    }
    if !request.protocols.iter().any(|token| token == METHOD) {
        return true;
    }

    let mut headers = request
        .headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case(PROTOCOL_HEADER));
    let Some((_, value)) = headers.next() else {
        return false;
    };
    if headers.next().is_some() {
        return false;
    }

    let mut tokens = value.split(',').map(str::trim);
    let [method, app, third, fourth, extra] = std::array::from_fn(|_| tokens.next());
    if method != Some(METHOD)
        || !app.is_some_and(|app| numbered_ascii(app, '1', 16, u8::is_ascii_alphanumeric))
        || extra.is_some()
    {
        return false;
    }

    let valid_id = |token| numbered_ascii(token, '2', 16, u8::is_ascii_alphanumeric);
    let valid_proof = |token| numbered_ascii(token, '3', 64, u8::is_ascii_hexdigit);
    match (third, fourth) {
        (None, None) => true,
        // Pass well-formed partial credentials to AppConnect for its typed rejection.
        (Some(token), None) => valid_id(token) || valid_proof(token),
        (Some(id), Some(proof)) => valid_id(id) && valid_proof(proof),
        _ => false,
    }
}

/// Validate the visitor offer before endpoint-libs echoes the first token.
/// Replay the unchanged head and any pipelined bytes to the ordinary upgrader.
pub(crate) async fn upgrade_visitor_handshake(
    mut stream: BoxedStream,
    addr: SocketAddr,
    config: &WsServerConfig,
) -> eyre::Result<Option<UpgradeEvent>> {
    let mut bytes = Vec::new();
    let mut chunk = [0; 2048];
    loop {
        if let Some(end) = head_end(&bytes) {
            if let Ok(Parsed::Upgrade(request)) = parse_request(&bytes[..end], DEFAULT_MAX_HEAD)
                && !visitor_offer_is_safe(&request)
            {
                stream
                    .write_all(b"HTTP/1.1 400 Bad Request\r\nConnection: close\r\nCache-Control: no-store\r\nContent-Length: 0\r\n\r\n")
                    .await?;
                return Ok(None);
            }
            break;
        }
        if bytes.len() > DEFAULT_MAX_HEAD {
            break;
        }
        let count = stream.read(&mut chunk).await?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..count]);
    }

    NagoWssUpgrader
        .upgrade_stream(
            Box::new(ReplayStream {
                prefix: Cursor::new(bytes),
                stream,
            }),
            addr,
            config,
        )
        .await
}

struct ReplayStream {
    prefix: Cursor<Vec<u8>>,
    stream: BoxedStream,
}

impl RawStream for ReplayStream {
    fn read<'a>(
        &'a mut self,
        buffer: &'a mut [u8],
    ) -> LocalBoxFuture<'a, Result<usize, StreamError>> {
        Box::pin(async move {
            let count = self.prefix.read(buffer).expect("cursor read cannot fail");
            if count != 0 {
                return Ok(count);
            }
            // The prefix is never needed once fully replayed. In particular,
            // don't keep the authentication header for the lifetime of a socket.
            self.prefix = Cursor::new(Vec::new());
            self.stream.read(buffer).await
        })
    }

    fn write_all<'a>(
        &'a mut self,
        buffer: &'a [u8],
    ) -> LocalBoxFuture<'a, Result<(), StreamError>> {
        self.stream.write_all(buffer)
    }
}
