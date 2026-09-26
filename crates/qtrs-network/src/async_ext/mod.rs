//! Asynchronous `Future` extensions bridging Qt Signal/Slot with modern Rust `async/await`.

use std::thread;

use qtrs_core::thread::future::{Future, Promise};

use crate::kernel::{HostAddress, HostInfo, SocketError};
use crate::tcp::{TcpServer, TcpSocket};
use crate::websocket::WebSocket;

/// Asynchronous extension for DNS host resolution.
pub struct DnsResolver;

impl DnsResolver {
    /// Resolves host asynchronously returning a `Future<Result<Vec<HostAddress>, SocketError>>`.
    pub fn lookup_host_async(host: impl Into<String>) -> Future<Result<Vec<HostAddress>, SocketError>> {
        let promise = Promise::new();
        let fut = promise.future();
        let h = host.into();

        thread::spawn(move || {
            let res = HostInfo::lookup_host(&h);
            promise.set_value(res);
        });

        fut
    }
}

/// Asynchronous extension for `TcpSocket`.
pub trait TcpSocketAsync {
    /// Connects to a host asynchronously.
    fn connect_async(host: &str, port: u16) -> Future<Result<TcpSocket, SocketError>>;
}

impl TcpSocketAsync for TcpSocket {
    fn connect_async(host: &str, port: u16) -> Future<Result<TcpSocket, SocketError>> {
        let promise = Promise::new();
        let fut = promise.future();
        let h = host.to_string();

        thread::spawn(move || {
            let sock = TcpSocket::new();
            match sock.connect_to_host(&h, port) {
                Ok(_) => promise.set_value(Ok(sock)),
                Err(e) => promise.set_value(Err(e)),
            }
        });

        fut
    }
}

/// Asynchronous extension for `TcpServer`.
pub trait TcpServerAsync {
    /// Accepts the next incoming connection asynchronously.
    fn accept_async(&self) -> Future<Result<TcpSocket, SocketError>>;
}

impl TcpServerAsync for TcpServer {
    fn accept_async(&self) -> Future<Result<TcpSocket, SocketError>> {
        let promise = Promise::new();
        let fut = promise.future();
        let server_clone = self.clone();

        thread::spawn(move || {
            loop {
                if let Some(conn) = server_clone.next_pending_connection() {
                    promise.set_value(Ok(conn));
                    break;
                }
                if !server_clone.is_listening() {
                    promise.set_value(Err(SocketError::OperationError));
                    break;
                }
                thread::sleep(std::time::Duration::from_millis(10));
            }
        });

        fut
    }
}

/// Asynchronous extension for `WebSocket`.
pub trait WebSocketAsync {
    /// Opens a WebSocket asynchronously.
    fn open_async(url: &str) -> Future<Result<WebSocket, SocketError>>;
}

impl WebSocketAsync for WebSocket {
    fn open_async(url: &str) -> Future<Result<WebSocket, SocketError>> {
        let promise = Promise::new();
        let fut = promise.future();
        let u = url.to_string();

        thread::spawn(move || {
            let ws = WebSocket::new();
            match ws.open(&u) {
                Ok(_) => promise.set_value(Ok(ws)),
                Err(e) => promise.set_value(Err(e)),
            }
        });

        fut
    }
}
