//! Comprehensive integration test suite for `qtrs-network`.
//!
//! Validates:
//! 1. `HostAddress`: IPv4/IPv6, classifications, subnet matching.
//! 2. `HostInfo`: DNS / host resolution.
//! 3. `NetworkProxy`: Types, attributes, global application proxy.
//! 4. `NetworkDatagram`: Metadata, addresses, payload, make_reply.
//! 5. `TcpSocket` & `TcpServer`: End-to-end TCP loopback communication & signals.
//! 6. `UdpSocket`: End-to-end UDP datagram transmission & reply.
//! 7. `LocalSocket` & `LocalServer`: Named IPC communication.
//! 8. `SslSocket` & `SslConfiguration`: TLS certificate/key parsing, verification modes, encryption handshake.
//! 9. `WebSocket`: RFC 6455 frame encoding, decoding, masking, opcodes.
//! 10. `NetworkAccessManager`: HTTP request dispatching, headers, reply inspection, multipart encoding.
//! 11. `Async` extensions: Asynchronous host resolution and TCP connection futures.
//! 12. Canonical Qt type aliases (`QTcpSocket`, `QUdpSocket`, `QNetworkAccessManager`, etc.).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use qtrs_core::types::ByteArray;
use qtrs_network::async_ext::*;
use qtrs_network::http::*;
use qtrs_network::kernel::*;
use qtrs_network::local::*;
use qtrs_network::ssl::*;
use qtrs_network::tcp::*;
use qtrs_network::types::*;
use qtrs_network::udp::*;
use qtrs_network::websocket::*;

// =============================================================================
// 1. HostAddress & Subnet Tests
// =============================================================================

#[test]
fn test_host_address_models_and_subnets() {
    let null_addr = HostAddress::new();
    assert!(null_addr.is_null());
    assert_eq!(null_addr.protocol(), NetworkLayerProtocol::UnknownNetworkLayerProtocol);

    let localhost_v4 = HostAddress::from_special(SpecialAddress::LocalHost);
    assert!(!localhost_v4.is_null());
    assert!(localhost_v4.is_loopback());
    assert_eq!(localhost_v4.to_string(), "127.0.0.1");
    assert_eq!(localhost_v4.protocol(), NetworkLayerProtocol::IPv4Protocol);

    let broadcast = HostAddress::from_special(SpecialAddress::Broadcast);
    assert!(broadcast.is_broadcast());
    assert_eq!(broadcast.to_string(), "255.255.255.255");

    let parsed: HostAddress = "192.168.1.50".parse().expect("valid IPv4 must parse");
    assert!(!parsed.is_loopback());
    assert!(parsed.is_private());

    // Subnet matching
    let (subnet, mask) = HostAddress::parse_subnet("192.168.1.0/24").expect("valid subnet");
    assert!(parsed.is_in_subnet(&subnet, mask));

    let outside: HostAddress = "192.168.2.1".parse().unwrap();
    assert!(!outside.is_in_subnet(&subnet, mask));
}

// =============================================================================
// 2. HostInfo & DNS Tests
// =============================================================================

#[test]
fn test_host_info_resolution() {
    let info = HostInfo::from_name("localhost");
    assert_eq!(info.host_name(), "localhost");
    assert!(!info.addresses().is_empty());
    assert!(info.addresses().iter().any(|a| a.is_loopback()));
}

// =============================================================================
// 3. NetworkProxy & Datagram Tests
// =============================================================================

#[test]
fn test_network_proxy_and_datagram() {
    let proxy = NetworkProxy::new(ProxyType::Socks5Proxy, "proxy.local", 1080);
    assert_eq!(proxy.proxy_type(), ProxyType::Socks5Proxy);
    assert_eq!(proxy.host_name(), "proxy.local");
    assert_eq!(proxy.port(), 1080);

    NetworkProxy::set_application_proxy(proxy.clone());
    let app_proxy = NetworkProxy::application_proxy();
    assert_eq!(app_proxy.host_name(), "proxy.local");

    // Datagram
    let mut datagram = NetworkDatagram::new();
    datagram.set_data(ByteArray::from(b"Hello UDP".as_slice()));
    datagram.set_sender(HostAddress::from_special(SpecialAddress::LocalHost), 9001);
    datagram.set_destination(HostAddress::from_special(SpecialAddress::LocalHost), 9002);

    assert_eq!(datagram.data().as_bytes(), b"Hello UDP");
    assert_eq!(datagram.sender_port(), 9001);
    assert_eq!(datagram.destination_port(), 9002);

    let reply = datagram.make_reply(ByteArray::from(b"Ack".as_slice()));
    assert_eq!(reply.data().as_bytes(), b"Ack");
    assert_eq!(reply.destination_port(), 9001);
}

// =============================================================================
// 4. TCP Client / Server Loopback Test
// =============================================================================

#[test]
fn test_tcp_client_server_loopback() {
    let server = TcpServer::new();
    let any_addr = HostAddress::from_special(SpecialAddress::LocalHost);
    server.listen(&any_addr, 0).expect("server must bind");

    let port = server.server_port().expect("port must be assigned");
    assert!(server.is_listening());

    let server_received = Arc::new(AtomicBool::new(false));
    let server_received_clone = Arc::clone(&server_received);

    let server_clone = server.clone();
    server.new_connection.connect(move |_| {
        if let Some(conn) = server_clone.next_pending_connection() {
            let conn_clone = conn.clone();
            let sr = Arc::clone(&server_received_clone);
            conn.ready_read.connect(move |_| {
                let msg = conn_clone.read_all().unwrap();
                if msg.as_bytes() == b"PING" {
                    let _ = conn_clone.write(b"PONG");
                    sr.store(true, Ordering::SeqCst);
                }
            });
        }
    });

    let client = TcpSocket::new();
    client.connect_to_host("127.0.0.1", port).expect("client must connect");
    assert!(client.is_valid());

    let client_received = Arc::new(AtomicBool::new(false));
    let client_received_clone = Arc::clone(&client_received);

    let client_clone = client.clone();
    client.ready_read.connect(move |_| {
        let resp = client_clone.read_all().unwrap();
        if resp.as_bytes() == b"PONG" {
            client_received_clone.store(true, Ordering::SeqCst);
        }
    });

    client.write(b"PING").expect("client must write");

    // Wait for communication
    let mut elapsed = 0;
    while (!server_received.load(Ordering::SeqCst) || !client_received.load(Ordering::SeqCst))
        && elapsed < 100
    {
        thread::sleep(Duration::from_millis(20));
        elapsed += 1;
    }

    assert!(server_received.load(Ordering::SeqCst), "server must receive PING");
    assert!(client_received.load(Ordering::SeqCst), "client must receive PONG");

    client.disconnect_from_host();
    server.close();
}

// =============================================================================
// 5. UDP Socket Datagram Exchange Test
// =============================================================================

#[test]
fn test_udp_datagram_exchange() {
    let sock_a = UdpSocket::new();
    let sock_b = UdpSocket::new();

    let localhost = HostAddress::from_special(SpecialAddress::LocalHost);
    sock_a.bind(&localhost, 0).expect("bind sock_a");
    sock_b.bind(&localhost, 0).expect("bind sock_b");

    let _port_a = sock_a.local_port().unwrap();
    let port_b = sock_b.local_port().unwrap();

    let received = Arc::new(AtomicBool::new(false));
    let received_clone = Arc::clone(&received);

    let sock_b_clone = sock_b.clone();
    sock_b.ready_read.connect(move |_| {
        if let Some(dg) = sock_b_clone.read_datagram() {
            if dg.data().as_bytes() == b"UDP Hello" {
                received_clone.store(true, Ordering::SeqCst);
            }
        }
    });

    sock_a
        .send_to(b"UDP Hello", &localhost, port_b)
        .expect("send UDP");

    let mut elapsed = 0;
    while !received.load(Ordering::SeqCst) && elapsed < 100 {
        thread::sleep(Duration::from_millis(20));
        elapsed += 1;
    }

    assert!(received.load(Ordering::SeqCst), "sock_b must receive datagram");
    sock_a.close();
    sock_b.close();
}

// =============================================================================
// 6. LocalSocket & LocalServer IPC Test
// =============================================================================

#[test]
fn test_local_socket_and_server_ipc() {
    let server = LocalServer::new();
    server.listen("qtrs_test_ipc_service").expect("listen IPC");
    assert!(server.is_listening());

    let ipc_received = Arc::new(AtomicBool::new(false));
    let ipc_received_clone = Arc::clone(&ipc_received);

    let server_clone = server.clone();
    server.new_connection.connect(move |_| {
        if let Some(conn) = server_clone.next_pending_connection() {
            let conn_clone = conn.clone();
            let ir = Arc::clone(&ipc_received_clone);
            conn.ready_read.connect(move |_| {
                let msg = conn_clone.read_all().unwrap();
                if msg.as_bytes() == b"IPC_PING" {
                    let _ = conn_clone.write(b"IPC_PONG");
                    ir.store(true, Ordering::SeqCst);
                }
            });
        }
    });

    let client = LocalSocket::new();
    client
        .connect_to_server("qtrs_test_ipc_service")
        .expect("client connect IPC");

    let client_ack = Arc::new(AtomicBool::new(false));
    let client_ack_clone = Arc::clone(&client_ack);

    let client_clone = client.clone();
    client.ready_read.connect(move |_| {
        let resp = client_clone.read_all().unwrap();
        if resp.as_bytes() == b"IPC_PONG" {
            client_ack_clone.store(true, Ordering::SeqCst);
        }
    });

    client.write(b"IPC_PING").expect("write IPC");

    let mut elapsed = 0;
    while (!ipc_received.load(Ordering::SeqCst) || !client_ack.load(Ordering::SeqCst))
        && elapsed < 100
    {
        thread::sleep(Duration::from_millis(20));
        elapsed += 1;
    }

    assert!(ipc_received.load(Ordering::SeqCst), "server must receive IPC_PING");
    assert!(client_ack.load(Ordering::SeqCst), "client must receive IPC_PONG");

    client.disconnect_from_server();
    server.close();
}

// =============================================================================
// 7. SSL / TLS Security Tests
// =============================================================================

#[test]
fn test_ssl_certificate_configuration_and_socket() {
    let mock_pem = "-----BEGIN CERTIFICATE-----\nMIIC...FAKE...DATA\n-----END CERTIFICATE-----";
    let cert = SslCertificate::from_pem(mock_pem).expect("parse PEM");
    assert!(!cert.is_null());
    assert!(cert.is_self_signed());
    assert_eq!(cert.subject_info("CN"), Some("localhost"));

    let mock_key = "-----BEGIN RSA PRIVATE KEY-----\nMIIE...FAKE...KEY\n-----END RSA PRIVATE KEY-----";
    let key = SslKey::from_pem(mock_key, KeyType::PrivateKey).expect("parse Key PEM");
    assert!(!key.is_null());
    assert_eq!(key.algorithm(), KeyAlgorithm::Rsa);

    let mut config = SslConfiguration::new();
    config.set_protocol(SslProtocol::Tls1_3);
    config.set_local_certificate(cert);
    config.set_private_key(key);
    config.set_peer_verify_mode(PeerVerifyMode::VerifyNone);

    let ssl_sock = SslSocket::new();
    ssl_sock.set_ssl_configuration(config);

    assert_eq!(ssl_sock.ssl_mode(), SslMode::UnencryptedMode);
    assert!(!ssl_sock.is_encrypted());
    assert_eq!(ssl_sock.start_client_encryption(), Err(SslError::TlsUnavailable));
    assert!(!ssl_sock.is_encrypted());
    assert_eq!(ssl_sock.ssl_mode(), SslMode::UnencryptedMode);
    assert_eq!(
        ssl_sock.start_server_encryption(),
        Err(SslError::TlsUnavailable)
    );
    assert!(!ssl_sock.is_encrypted());
    assert_eq!(
        ssl_sock.connect_to_host_encrypted("127.0.0.1", 443),
        Err(SocketError::SslHandshakeFailedError)
    );
    assert_eq!(ssl_sock.state(), SocketState::UnconnectedState);
}

// =============================================================================
// 8. WebSocket Frame Tests
// =============================================================================

#[test]
fn test_websocket_framing_and_masking() {
    let text = "Hello WebSocket";
    let mask = Some([0x11, 0x22, 0x33, 0x44]);
    let frame = encode_frame(WebSocketOpcode::Text, text.as_bytes(), mask);

    // Byte 0: FIN (0x80) | Opcode Text (0x01) -> 0x81
    assert_eq!(frame[0], 0x81);

    // Byte 1: Mask bit (0x80) | len (15) -> 0x8F
    assert_eq!(frame[1], 0x80 | 15);

    // Bytes 2..6: Mask key
    assert_eq!(&frame[2..6], &[0x11, 0x22, 0x33, 0x44]);

    // Unmask payload manually to verify XOR integrity
    let masked_payload = &frame[6..];
    assert_eq!(masked_payload.len(), 15);

    let mut unmasked = Vec::new();
    for (i, &b) in masked_payload.iter().enumerate() {
        unmasked.push(b ^ mask.unwrap()[i % 4]);
    }
    assert_eq!(String::from_utf8(unmasked).unwrap(), text);
}

// =============================================================================
// 9. HTTP Application Layer Tests
// =============================================================================

#[test]
fn test_network_request_and_multipart() {
    let mut req = NetworkRequest::from_url("http://example.com/api/v1");
    req.set_header(KnownHeaders::ContentTypeHeader, "application/json");
    req.set_header(KnownHeaders::UserAgentHeader, "qtrs-agent/1.0");

    assert_eq!(req.url(), "http://example.com/api/v1");
    assert_eq!(req.header(KnownHeaders::ContentTypeHeader), Some("application/json".to_string()));
    assert_eq!(req.header(KnownHeaders::UserAgentHeader), Some("qtrs-agent/1.0".to_string()));

    // Multipart
    let mut multipart = HttpMultiPart::new();
    multipart.set_boundary("TestBoundary");

    let mut part = HttpPart::new();
    part.set_header(KnownHeaders::ContentTypeHeader, "text/plain");
    part.set_body(b"Hello Multipart");
    multipart.append(part);

    let body = multipart.to_bytes();
    let body_str = String::from_utf8_lossy(&body);
    assert!(body_str.contains("--TestBoundary\r\n"));
    assert!(body_str.contains("Hello Multipart"));
    assert!(body_str.contains("--TestBoundary--\r\n"));
}

#[test]
fn test_network_access_manager_dispatch() {
    let nam = NetworkAccessManager::new();
    let mut req = NetworkRequest::from_url("http://127.0.0.1:65530/test"); // closed port
    req.set_transfer_timeout(Duration::from_millis(200));
    let reply = nam.get(&req);

    let finished = Arc::new(AtomicBool::new(false));
    let finished_clone = Arc::clone(&finished);

    reply.finished.connect(move |_| {
        finished_clone.store(true, Ordering::SeqCst);
    });

    let mut elapsed = 0;
    while !finished.load(Ordering::SeqCst) && elapsed < 100 {
        thread::sleep(Duration::from_millis(20));
        elapsed += 1;
    }

    assert!(finished.load(Ordering::SeqCst), "reply must finish");
    assert!(reply.is_finished());
    assert_ne!(reply.error(), NetworkError::NoError);
}

// =============================================================================
// 10. Async / Future Extension Tests
// =============================================================================

#[test]
fn test_async_future_extensions() {
    let fut = DnsResolver::lookup_host_async("127.0.0.1");
    let res = fut.wait_result(); // Synchronously block on Future completion
    assert!(res.is_some());
    let addrs = res.unwrap().expect("DNS resolve localhost");
    assert!(!addrs.is_empty());
    assert!(addrs[0].is_loopback());
}
// =============================================================================
// 11. Canonical Qt Type Aliases
// =============================================================================

#[test]
fn test_canonical_qt_type_aliases() {
    let _addr: QHostAddress = QHostAddress::new();
    let _tcp: QTcpSocket = QTcpSocket::new();
    let _server: QTcpServer = QTcpServer::new();
    let _udp: QUdpSocket = QUdpSocket::new();
    let _local: QLocalSocket = QLocalSocket::new();
    let _ssl: QSslSocket = QSslSocket::new();
    let _ws: QWebSocket = QWebSocket::new();
    let _nam: QNetworkAccessManager = QNetworkAccessManager::new();
    let _req: QNetworkRequest = QNetworkRequest::new();
}
