//! RFC 6455 compliant WebSocket implementation matching Qt's `QWebSocket`.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use qtrs_core::signal::Signal;
use qtrs_core::types::ByteArray;

use base64::Engine;
use sha1::{Digest, Sha1};
use crate::kernel::{SocketError, SocketState};

// =============================================================================
// WebSocket Opcodes & Framing
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSocketOpcode {
    Continuation = 0x0,
    Text = 0x1,
    Binary = 0x2,
    Close = 0x8,
    Ping = 0x9,
    Pong = 0xA,
}

impl WebSocketOpcode {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0x0 => Some(Self::Continuation),
            0x1 => Some(Self::Text),
            0x2 => Some(Self::Binary),
            0x8 => Some(Self::Close),
            0x9 => Some(Self::Ping),
            0xA => Some(Self::Pong),
            _ => None,
        }
    }
}

/// Encodes an RFC 6455 WebSocket frame.
pub fn encode_frame(
    opcode: WebSocketOpcode,
    payload: &[u8],
    mask: Option<[u8; 4]>,
) -> Vec<u8> {
    let mut frame = Vec::new();
    let fin = 0x80u8;
    let b0 = fin | (opcode as u8 & 0x0F);
    frame.push(b0);

    let mask_bit = if mask.is_some() { 0x80u8 } else { 0x00u8 };
    let len = payload.len();

    if len <= 125 {
        frame.push(mask_bit | (len as u8));
    } else if len <= 65535 {
        frame.push(mask_bit | 126);
        frame.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        frame.push(mask_bit | 127);
        frame.extend_from_slice(&(len as u64).to_be_bytes());
    }

    if let Some(mask_key) = mask {
        frame.extend_from_slice(&mask_key);
        let mut masked = Vec::with_capacity(len);
        for (i, &b) in payload.iter().enumerate() {
            masked.push(b ^ mask_key[i % 4]);
        }
        frame.extend_from_slice(&masked);
    } else {
        frame.extend_from_slice(payload);
    }

    frame
}

// =============================================================================
// WebSocket Client & Peer (QWebSocket)
// =============================================================================

struct WebSocketInner {
    tcp: Option<TcpStream>,
    url: String,
    state: SocketState,
    worker_stop: Arc<AtomicBool>,
}

/// WebSocket client implementation matching `QWebSocket`.
#[derive(Clone)]
pub struct WebSocket {
    inner: Arc<Mutex<WebSocketInner>>,

    // Qt Signals
    pub connected: Signal<()>,
    pub disconnected: Signal<()>,
    pub text_message_received: Signal<String>,
    pub binary_message_received: Signal<ByteArray>,
    pub pong: Signal<u64>,
    pub error_occurred: Signal<SocketError>,
    pub state_changed: Signal<SocketState>,
}

impl Default for WebSocket {
    fn default() -> Self {
        Self::new()
    }
}

impl WebSocket {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(WebSocketInner {
                tcp: None,
                url: String::new(),
                state: SocketState::UnconnectedState,
                worker_stop: Arc::new(AtomicBool::new(false)),
            })),
            connected: Signal::new(),
            disconnected: Signal::new(),
            text_message_received: Signal::new(),
            binary_message_received: Signal::new(),
            pong: Signal::new(),
            error_occurred: Signal::new(),
            state_changed: Signal::new(),
        }
    }

    /// Opens a connection to a WebSocket URL (`ws://host:port/path`).
    pub fn open(&self, url: &str) -> Result<(), SocketError> {
        self.close(1000, "Normal closure");

        let (host, port, path) = parse_ws_url(url)?;
        self.set_state(SocketState::ConnectingState);

        let target = format!("{}:{}", host, port);
        let stream = TcpStream::connect(target)?;
        stream.set_nodelay(true)?;

        // RFC 6455 requires a fresh, unpredictable 16-byte client nonce.
        let mut nonce = [0u8; 16];
        getrandom::getrandom(&mut nonce).map_err(|_| SocketError::OperationError)?;
        let sec_key = base64::engine::general_purpose::STANDARD.encode(nonce);
        let expected_accept = websocket_accept(&sec_key);
        let mut client_stream = stream;
        let handshake_req = format!(
            "GET {} HTTP/1.1\r\n\
             Host: {}:{}\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Key: {}\r\n\
             Sec-WebSocket-Version: 13\r\n\r\n",
            path, host, port, sec_key
        );

        client_stream.write_all(handshake_req.as_bytes())?;

        // Read the complete response header without consuming any following frame bytes.
        let mut response = Vec::new();
        let mut byte = [0u8; 1];
        while response.len() < 16 * 1024 && !response.ends_with(b"\r\n\r\n") {
            client_stream.read_exact(&mut byte)?;
            response.push(byte[0]);
        }
        if !response.ends_with(b"\r\n\r\n") {
            self.set_state(SocketState::UnconnectedState);
            return Err(SocketError::NetworkError);
        }
        let response = std::str::from_utf8(&response).map_err(|_| SocketError::NetworkError)?;
        let mut lines = response.split("\r\n");
        let valid_status = lines.next() == Some("HTTP/1.1 101 Switching Protocols");
        let mut upgrade = false;
        let mut connection_upgrade = false;
        let mut accept = false;
        for line in lines {
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            match name.trim().to_ascii_lowercase().as_str() {
                "upgrade" => upgrade = value.trim().eq_ignore_ascii_case("websocket"),
                "connection" => {
                    connection_upgrade = value
                        .split(',')
                        .any(|token| token.trim().eq_ignore_ascii_case("upgrade"));
                }
                "sec-websocket-accept" => {
                    accept = value.trim() == expected_accept
                }
                _ => {}
            }
        }
        if !(valid_status && upgrade && connection_upgrade && accept) {
            self.set_state(SocketState::UnconnectedState);
            self.error_occurred.emit(&SocketError::NetworkError);
            return Err(SocketError::NetworkError);
        }
        {
            let mut lock = self.inner.lock().unwrap();
            lock.url = url.to_string();
            lock.state = SocketState::ConnectedState;
        }

        self.start_message_reader(client_stream);
        self.state_changed.emit(&SocketState::ConnectedState);
        self.connected.emit(&());
        Ok(())
    }

    /// Sends a text message frame (`sendTextMessage`).
    pub fn send_text_message(&self, message: &str) -> Result<usize, SocketError> {
        let mask = Some(random_mask()?);
        let frame = encode_frame(WebSocketOpcode::Text, message.as_bytes(), mask);
        self.write_raw(&frame)
    }

    /// Sends a binary message frame (`sendBinaryMessage`).
    pub fn send_binary_message(&self, message: &[u8]) -> Result<usize, SocketError> {
        let mask = Some(random_mask()?);
        let frame = encode_frame(WebSocketOpcode::Binary, message, mask);
        self.write_raw(&frame)
    }

    /// Sends a ping frame (`ping`).
    pub fn ping(&self, payload: &[u8]) -> Result<usize, SocketError> {
        if payload.len() > 125 {
            return Err(SocketError::OperationError);
        }
        let mask = Some(random_mask()?);
        let frame = encode_frame(WebSocketOpcode::Ping, payload, mask);
        self.write_raw(&frame)
    }

    /// Closes the WebSocket connection (`close`).
    pub fn close(&self, code: u16, reason: &str) {
        {
            let mut lock = self.inner.lock().unwrap();
            if lock.state == SocketState::ConnectedState {
                let mut payload = Vec::new();
                payload.extend_from_slice(&code.to_be_bytes());
                payload.extend_from_slice(reason.as_bytes());
                if valid_close_code(code) && payload.len() <= 125 {
                    if let Ok(mask) = random_mask() {
                        let frame = encode_frame(WebSocketOpcode::Close, &payload, Some(mask));
                        if let Some(stream) = lock.tcp.as_mut() {
                            let _ = stream.write_all(&frame);
                        }
                    }
                }
            }

            lock.worker_stop.store(true, Ordering::SeqCst);
            lock.tcp = None;
            lock.state = SocketState::UnconnectedState;
        }
        self.state_changed.emit(&SocketState::UnconnectedState);
        self.disconnected.emit(&());
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        self.state() == SocketState::ConnectedState
    }

    #[inline]
    pub fn state(&self) -> SocketState {
        self.inner.lock().unwrap().state
    }

    fn write_raw(&self, data: &[u8]) -> Result<usize, SocketError> {
        let mut lock = self.inner.lock().unwrap();
        let stream = lock.tcp.as_mut().ok_or(SocketError::OperationError)?;
        stream.write_all(data)?;
        Ok(data.len())
    }

    fn set_state(&self, new_state: SocketState) {
        let mut lock = self.inner.lock().unwrap();
        if lock.state != new_state {
            lock.state = new_state;
            drop(lock);
            self.state_changed.emit(&new_state);
        }
    }

    fn start_message_reader(&self, stream: TcpStream) {
        let (stream_clone, stop_flag) = {
            let mut lock = self.inner.lock().unwrap();
            lock.worker_stop.store(true, Ordering::SeqCst);
            lock.worker_stop = Arc::new(AtomicBool::new(false));

            let c = match stream.try_clone() {
                Ok(cl) => cl,
                Err(_) => return,
            };
            lock.tcp = Some(stream);
            (c, Arc::clone(&lock.worker_stop))
        };

        let text_sig = self.text_message_received.clone();
        let bin_sig = self.binary_message_received.clone();
        let pong_sig = self.pong.clone();
        let disc_sig = self.disconnected.clone();

        thread::spawn(move || {
            let mut reader = stream_clone;

            let mut fragmented: Option<(WebSocketOpcode, Vec<u8>)> = None;
            while !stop_flag.load(Ordering::Relaxed) {
                let (fin, opcode, payload) = match read_frame(&mut reader) {
                    Ok(frame) => frame,
                    Err(_) => break,
                };

                match opcode {
                    WebSocketOpcode::Text | WebSocketOpcode::Binary => {
                        if fragmented.is_some() {
                            break;
                        }
                        if fin {
                            if opcode == WebSocketOpcode::Text {
                                let Ok(text) = String::from_utf8(payload) else {
                                    break;
                                };
                                text_sig.emit(&text);
                            } else {
                                bin_sig.emit(&ByteArray::from(payload));
                            }
                        } else {
                            fragmented = Some((opcode, payload));
                        }
                    }
                    WebSocketOpcode::Continuation => {
                        let Some((_, data)) = fragmented.as_mut() else {
                            break;
                        };
                        if data.len().saturating_add(payload.len()) > 64 * 1024 * 1024 {
                            break;
                        }
                        data.extend_from_slice(&payload);
                        if fin {
                            let (message_opcode, data) = fragmented.take().unwrap();
                            if message_opcode == WebSocketOpcode::Text {
                                let Ok(text) = String::from_utf8(data) else {
                                    break;
                                };
                                text_sig.emit(&text);
                            } else {
                                bin_sig.emit(&ByteArray::from(data));
                            }
                        }
                    }
                    WebSocketOpcode::Ping => {
                        let pong_frame = encode_frame(WebSocketOpcode::Pong, &payload, None);
                        let _ = reader.write_all(&pong_frame);
                    }
                    WebSocketOpcode::Pong => pong_sig.emit(&0),
                    WebSocketOpcode::Close => {
                        if let Ok(mask) = random_mask() {
                            let frame = encode_frame(WebSocketOpcode::Close, &payload, Some(mask));
                            let _ = reader.write_all(&frame);
                        }
                        break;
                    }
                }
            }

            disc_sig.emit(&());
        });
    }
}

fn websocket_accept(key: &str) -> String {
    let mut hash = Sha1::new();
    hash.update(key.as_bytes());
    hash.update(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    base64::engine::general_purpose::STANDARD.encode(hash.finalize())
}

fn random_mask() -> Result<[u8; 4], SocketError> {
    let mut mask = [0; 4];
    getrandom::getrandom(&mut mask).map_err(|_| SocketError::OperationError)?;
    Ok(mask)
}

fn parse_ws_url(url: &str) -> Result<(String, u16, String), SocketError> {

    // Secure WebSockets require TLS, which this implementation does not provide.
    let stripped = url
        .strip_prefix("ws://")
        .ok_or(SocketError::OperationError)?;

    let (host_port, path) = match stripped.split_once('/') {
        Some((hp, p)) => (hp, format!("/{}", p)),
        None => (stripped, "/".to_string()),
    };
    if host_port.is_empty()
        || host_port.contains('@')
        || host_port.contains('#')
        || host_port.contains('?')
    {
        return Err(SocketError::HostNotFoundError);
    }
    let (host, port) = match host_port.rsplit_once(':') {
        Some((h, p)) if !h.contains(':') => {
            let parsed_port = p.parse::<u16>().map_err(|_| SocketError::HostNotFoundError)?;
            (h.to_string(), parsed_port)
        }
        Some(_) => return Err(SocketError::HostNotFoundError),
        None => (host_port.to_string(), 80),
    };
    if host.is_empty() || port == 0 {
        return Err(SocketError::HostNotFoundError);
    }
    Ok((host, port, path))
}
fn valid_close_code(code: u16) -> bool {
    matches!(code, 1000..=1014 if !matches!(code, 1004 | 1005 | 1006))
        || (3000..=4999).contains(&code)
}
fn valid_close_payload(payload: &[u8]) -> bool {
    match payload.len() {
        0 => true,
        1 => false,
        _ => {
            let code = u16::from_be_bytes([payload[0], payload[1]]);
            valid_close_code(code)
                && code != 1010
                && std::str::from_utf8(&payload[2..]).is_ok()
        }
    }
}

/// Reads one server-to-client frame and rejects non-canonical or forbidden wire forms.
fn read_frame(reader: &mut impl Read) -> std::io::Result<(bool, WebSocketOpcode, Vec<u8>)> {
    const MAX_FRAME_SIZE: u64 = 64 * 1024 * 1024;
    let mut header = [0u8; 2];
    reader.read_exact(&mut header)?;
    let fin = header[0] & 0x80 != 0;
    if header[0] & 0x70 != 0 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "RSV bits set"));
    }
    let opcode = WebSocketOpcode::from_u8(header[0] & 0x0f)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "reserved opcode"))?;
    if header[1] & 0x80 != 0 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "server frame is masked"));
    }
    let marker = header[1] & 0x7f;
    let length = match marker {
        0..=125 => marker as u64,
        126 => {
            let mut bytes = [0; 2];
            reader.read_exact(&mut bytes)?;
            let n = u16::from_be_bytes(bytes) as u64;
            if n < 126 {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "non-canonical length"));
            }
            n
        }
        127 => {
            let mut bytes = [0; 8];
            reader.read_exact(&mut bytes)?;
            let n = u64::from_be_bytes(bytes);
            if n < 65536 || n >> 63 != 0 {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid 64-bit length"));
            }
            n
        }
        _ => unreachable!(),
    };
    let control = (opcode as u8) & 0x08 != 0;
    if (control && (!fin || length > 125)) || length > MAX_FRAME_SIZE {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid frame bounds"));
    }
    let mut payload = vec![0; length as usize];
    reader.read_exact(&mut payload)?;
    if opcode == WebSocketOpcode::Close && !valid_close_payload(&payload) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid close payload",
        ));
    }
    Ok((fin, opcode, payload))
}


#[cfg(test)]
mod control_tests {
    use super::*;

    #[test]
    fn handshake_accept_matches_rfc_example() {
        assert_eq!(
            websocket_accept("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }

    #[test]
    fn control_payloads_and_close_codes_follow_rfc_bounds() {
        let socket = WebSocket::new();
        assert!(socket.ping(&[0; 126]).is_err());
        assert!(valid_close_code(1000));
        assert!(!valid_close_code(1005));
        assert!(!valid_close_code(2000));
        assert!(valid_close_code(3000));
    }

    #[test]
    fn open_validates_the_upgrade_accept_token() {
        fn open_with_accept(valid_accept: bool) -> (bool, SocketState) {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = Vec::new();
                let mut byte = [0; 1];
                while !request.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                }
                let request = String::from_utf8(request).unwrap();
                let key = request
                    .lines()
                    .find_map(|line| line.strip_prefix("Sec-WebSocket-Key: "))
                    .unwrap();
                let accept = if valid_accept {
                    websocket_accept(key)
                } else {
                    "invalid".to_owned()
                };
                write!(
                    stream,
                    "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
                )
                .unwrap();
            });

            let socket = WebSocket::new();
            let connected = socket.open(&format!("ws://{address}/chat")).is_ok();
            let state = socket.state();
            server.join().unwrap();
            socket.close(1000, "");
            (connected, state)
        }

        assert_eq!(
            open_with_accept(true),
            (true, SocketState::ConnectedState)
        );
        assert_eq!(
            open_with_accept(false),
            (false, SocketState::UnconnectedState)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn frame_parser_rejects_invalid_wire_forms() {
        let invalid_frames: &[&[u8]] = &[
            &[0xC1, 0x00], // RSV bit set
            &[0x83, 0x00], // reserved opcode
            &[0x81, 0x80, 0, 0, 0, 0], // server-to-client frame masked
            &[0x81, 126, 0, 125], // non-canonical extended length
            &[0x89, 126, 0, 126], // control frame too large
            &[0x09, 0], // fragmented control frame
        ];
        for bytes in invalid_frames {
            assert!(read_frame(&mut Cursor::new(bytes)).is_err(), "{bytes:?}");
        }
    }

    #[test]
    fn close_frames_validate_status_and_utf8_reason() {
        for bytes in [
            &[0x88, 0x01, 0x00][..], // A one-byte close payload is forbidden.
            &[0x88, 0x02, 0x03, 0xed][..], // 1005 is a reserved status code.
            &[0x88, 0x04, 0x03, 0xe8, 0xff, 0xff][..], // Close reason is not UTF-8.
        ] {
            assert!(read_frame(&mut Cursor::new(bytes)).is_err(), "{bytes:?}");
        }
        assert_eq!(
            read_frame(&mut Cursor::new([0x88, 0x00])).unwrap(),
            (true, WebSocketOpcode::Close, Vec::new())
        );
    }

    #[test]
    fn frame_parser_accepts_fragmented_data_and_binary_lengths() {
        let bytes = [0x01, 0x02, b'h', b'i'];
        assert_eq!(
            read_frame(&mut Cursor::new(bytes)).unwrap(),
            (false, WebSocketOpcode::Text, b"hi".to_vec())
        );
        let mut bytes = vec![0x82, 126, 0, 126];
        bytes.extend_from_slice(&[7; 126]);
        let (fin, opcode, payload) = read_frame(&mut Cursor::new(bytes)).unwrap();
        assert!(fin);
        assert_eq!(opcode, WebSocketOpcode::Binary);
        assert_eq!(payload, vec![7; 126]);
    }

    #[test]
    fn secure_websocket_urls_are_not_downgraded() {
        assert!(parse_ws_url("wss://example.test/").is_err());
    }
}
