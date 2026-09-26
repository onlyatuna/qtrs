//! Canonical Qt-to-Rust type aliases for `qtrs-network`.

pub type QHostAddress = crate::kernel::HostAddress;
pub type QHostInfo = crate::kernel::HostInfo;
pub type QNetworkProxy = crate::kernel::NetworkProxy;
pub type QNetworkDatagram = crate::kernel::NetworkDatagram;

pub type QTcpSocket = crate::tcp::TcpSocket;
pub type QTcpServer = crate::tcp::TcpServer;

pub type QUdpSocket = crate::udp::UdpSocket;

pub type QLocalSocket = crate::local::LocalSocket;
pub type QLocalServer = crate::local::LocalServer;

pub type QSslSocket = crate::ssl::SslSocket;
pub type QSslConfiguration = crate::ssl::SslConfiguration;
pub type QSslCertificate = crate::ssl::SslCertificate;
pub type QSslKey = crate::ssl::SslKey;
pub type QSslError = crate::ssl::SslError;

pub type QWebSocket = crate::websocket::WebSocket;

pub type QNetworkAccessManager = crate::http::NetworkAccessManager;
pub type QNetworkRequest = crate::http::NetworkRequest;
pub type QNetworkReply = crate::http::NetworkReply;
pub type QHttpMultiPart = crate::http::HttpMultiPart;
pub type QHttpPart = crate::http::HttpPart;
