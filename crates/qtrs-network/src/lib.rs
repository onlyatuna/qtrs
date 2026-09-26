//! `qtrs-network` - High-performance Qt-equivalent Network Subsystem for Rust.
//!
//! Provides QtNetwork parity structured across layered modules:
//! - [`kernel`]: Host address models (`QHostAddress`), DNS info (`QHostInfo`), proxies (`QNetworkProxy`),
//!   datagrams (`QNetworkDatagram`), and socket states/errors.
//! - [`tcp`]: TCP client socket (`QTcpSocket`) and TCP listening server (`QTcpServer`).
//! - [`udp`]: UDP socket (`QUdpSocket`) with multicast and datagram transmission.
//! - [`local`]: Cross-platform IPC local socket (`QLocalSocket`) and server (`QLocalServer`).
//! - [`ssl`]: TLS/SSL abstraction (`QSslSocket`, `QSslConfiguration`, `QSslCertificate`, `QSslKey`).
//! - [`websocket`]: RFC 6455 WebSocket client/server (`QWebSocket`).
//! - [`http`]: High-level HTTP engine (`QNetworkAccessManager`, `QNetworkRequest`, `QNetworkReply`).
//! - [`async_ext`]: Async/await & `Future` bridge extensions.
//! - [`types`]: Canonical Qt-to-Rust type aliases.

pub mod async_ext;
pub mod http;
pub mod kernel;
pub mod local;
pub mod ssl;
pub mod tcp;
pub mod types;
pub mod udp;
pub mod websocket;

// Re-export core types for ergonomic top-level use
pub use async_ext::*;
pub use http::*;
pub use kernel::*;
pub use local::*;
pub use ssl::*;
pub use tcp::*;
pub use types::*;
pub use udp::*;
pub use websocket::*;
