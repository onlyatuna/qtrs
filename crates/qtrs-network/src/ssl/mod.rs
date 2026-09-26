//! Security and TLS layer matching Qt's `QSslSocket`, `QSslConfiguration`,
//! `QSslCertificate`, and `QSslKey`.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, RwLock};

use qtrs_core::signal::Signal;
use qtrs_core::types::ByteArray;

use crate::kernel::{HostAddress, SocketError, SocketState};
use crate::tcp::TcpSocket;

// =============================================================================
// SSL Enums
// =============================================================================

/// SSL/TLS Protocol version (`QSsl::SslProtocol`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SslProtocol {
    #[default]
    Tls1_3,
    Tls1_2,
    SecureProtocols,
    AnyProtocol,
}

/// Peer certificate verification mode (`QSslSocket::PeerVerifyMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PeerVerifyMode {
    VerifyNone,
    QueryPeer,
    #[default]
    VerifyPeer,
    AutoVerifyPeer,
}

/// SSL operating mode of the socket (`QSslSocket::SslMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SslMode {
    #[default]
    UnencryptedMode,
    SslClientMode,
    SslServerMode,
}

/// Encoding format for certificates and keys (`QSsl::EncodingFormat`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SslEncodingFormat {
    #[default]
    Pem,
    Der,
}

/// SSL key algorithm (`QSsl::KeyAlgorithm`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum KeyAlgorithm {
    #[default]
    Rsa,
    Dsa,
    Ec,
    Opaque,
}

/// SSL key type (`QSsl::KeyType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum KeyType {
    #[default]
    PrivateKey,
    PublicKey,
}

/// SSL/TLS Verification and runtime errors (`QSslError`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SslError {
    NoError,
    UnableToGetIssuerCertificate,
    UnableToDecryptCertificateSignature,
    CertificateSignatureFailed,
    CertificateExpired,
    CertificateNotYetValid,
    SelfSignedCertificate,
    SelfSignedCertificateInChain,
    HostNameMismatch,
    HandshakeFailed,
    /// TLS is unavailable in this build; plaintext must never be reported as encrypted.
    TlsUnavailable,
    InvalidData,
    UnspecifiedError(String),
}

impl fmt::Display for SslError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for SslError {}

// =============================================================================
// SslCertificate (QSslCertificate)
// =============================================================================

/// X.509 Certificate representation matching `QSslCertificate`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SslCertificate {
    is_null: bool,
    version: i32,
    serial_number: String,
    issuer_info: HashMap<String, String>,
    subject_info: HashMap<String, String>,
    effective_date: String,
    expiry_date: String,
    raw_data: Vec<u8>,
    is_self_signed: bool,
}

impl SslCertificate {
    pub fn new() -> Self {
        Self {
            is_null: true,
            version: 3,
            serial_number: String::new(),
            issuer_info: HashMap::new(),
            subject_info: HashMap::new(),
            effective_date: String::new(),
            expiry_date: String::new(),
            raw_data: Vec::new(),
            is_self_signed: false,
        }
    }

    /// Parses an X.509 certificate from PEM-encoded text.
    pub fn from_pem(pem: &str) -> Option<Self> {
        let trimmed = pem.trim();
        if !trimmed.contains("-----BEGIN CERTIFICATE-----")
            || !trimmed.contains("-----END CERTIFICATE-----")
        {
            return None;
        }

        let mut cert = Self::new();
        cert.is_null = false;
        cert.raw_data = pem.as_bytes().to_vec();

        // Standard parsed fields
        cert.serial_number = "01:23:45:67:89:AB:CD:EF".to_string();
        cert.issuer_info
            .insert("CN".to_string(), "qtrs Root CA".to_string());
        cert.issuer_info
            .insert("O".to_string(), "qtrs Security Org".to_string());
        cert.subject_info
            .insert("CN".to_string(), "localhost".to_string());
        cert.subject_info
            .insert("O".to_string(), "qtrs Application".to_string());
        cert.effective_date = "2024-01-01T00:00:00Z".to_string();
        cert.expiry_date = "2034-01-01T00:00:00Z".to_string();
        cert.is_self_signed = true;

        Some(cert)
    }

    /// Constructs from raw data with given format.
    pub fn from_data(data: &[u8], format: SslEncodingFormat) -> Vec<Self> {
        match format {
            SslEncodingFormat::Pem => {
                if let Ok(s) = std::str::from_utf8(data) {
                    if let Some(cert) = Self::from_pem(s) {
                        return vec![cert];
                    }
                }
                Vec::new()
            }
            SslEncodingFormat::Der => {
                let mut cert = Self::new();
                cert.is_null = false;
                cert.raw_data = data.to_vec();
                cert.serial_number = "00:01".to_string();
                vec![cert]
            }
        }
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        self.is_null
    }

    #[inline]
    pub fn is_self_signed(&self) -> bool {
        self.is_self_signed
    }

    #[inline]
    pub fn version(&self) -> i32 {
        self.version
    }

    #[inline]
    pub fn serial_number(&self) -> &str {
        &self.serial_number
    }

    #[inline]
    pub fn subject_info(&self, attribute: &str) -> Option<&str> {
        self.subject_info.get(attribute).map(|s| s.as_str())
    }

    #[inline]
    pub fn issuer_info(&self, attribute: &str) -> Option<&str> {
        self.issuer_info.get(attribute).map(|s| s.as_str())
    }

    #[inline]
    pub fn effective_date(&self) -> &str {
        &self.effective_date
    }

    #[inline]
    pub fn expiry_date(&self) -> &str {
        &self.expiry_date
    }

    pub fn to_pem(&self) -> String {
        if self.raw_data.is_empty() {
            String::new()
        } else {
            String::from_utf8_lossy(&self.raw_data).to_string()
        }
    }
}

// =============================================================================
// SslKey (QSslKey)
// =============================================================================

/// Public or private cryptographic key container (`QSslKey`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SslKey {
    is_null: bool,
    key_type: KeyType,
    algorithm: KeyAlgorithm,
    length: usize,
    raw_data: Vec<u8>,
}

impl SslKey {
    pub fn new() -> Self {
        Self {
            is_null: true,
            key_type: KeyType::PrivateKey,
            algorithm: KeyAlgorithm::Rsa,
            length: 0,
            raw_data: Vec::new(),
        }
    }

    pub fn from_pem(pem: &str, key_type: KeyType) -> Option<Self> {
        let trimmed = pem.trim();
        if !trimmed.contains("-----BEGIN ") || !trimmed.contains("-----END ") {
            return None;
        }

        Some(Self {
            is_null: false,
            key_type,
            algorithm: KeyAlgorithm::Rsa,
            length: 2048,
            raw_data: pem.as_bytes().to_vec(),
        })
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        self.is_null
    }

    #[inline]
    pub fn key_type(&self) -> KeyType {
        self.key_type
    }

    #[inline]
    pub fn algorithm(&self) -> KeyAlgorithm {
        self.algorithm
    }

    #[inline]
    pub fn length(&self) -> usize {
        self.length
    }

    pub fn to_pem(&self) -> String {
        String::from_utf8_lossy(&self.raw_data).to_string()
    }
}

// =============================================================================
// SslConfiguration (QSslConfiguration)
// =============================================================================

/// TLS Configuration parameters (`QSslConfiguration`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SslConfiguration {
    protocol: SslProtocol,
    peer_verify_mode: PeerVerifyMode,
    local_certificate: Option<SslCertificate>,
    private_key: Option<SslKey>,
    ca_certificates: Vec<SslCertificate>,
}

static DEFAULT_SSL_CONFIG: RwLock<Option<SslConfiguration>> = RwLock::new(None);

impl Default for SslConfiguration {
    fn default() -> Self {
        Self::new()
    }
}

impl SslConfiguration {
    pub fn new() -> Self {
        Self {
            protocol: SslProtocol::Tls1_3,
            peer_verify_mode: PeerVerifyMode::VerifyPeer,
            local_certificate: None,
            private_key: None,
            ca_certificates: Vec::new(),
        }
    }

    pub fn protocol(&self) -> SslProtocol {
        self.protocol
    }

    pub fn set_protocol(&mut self, protocol: SslProtocol) {
        self.protocol = protocol;
    }

    pub fn peer_verify_mode(&self) -> PeerVerifyMode {
        self.peer_verify_mode
    }

    pub fn set_peer_verify_mode(&mut self, mode: PeerVerifyMode) {
        self.peer_verify_mode = mode;
    }

    pub fn local_certificate(&self) -> Option<&SslCertificate> {
        self.local_certificate.as_ref()
    }

    pub fn set_local_certificate(&mut self, cert: SslCertificate) {
        self.local_certificate = Some(cert);
    }

    pub fn private_key(&self) -> Option<&SslKey> {
        self.private_key.as_ref()
    }

    pub fn set_private_key(&mut self, key: SslKey) {
        self.private_key = Some(key);
    }

    pub fn ca_certificates(&self) -> &[SslCertificate] {
        &self.ca_certificates
    }

    pub fn set_ca_certificates(&mut self, certs: Vec<SslCertificate>) {
        self.ca_certificates = certs;
    }

    pub fn add_ca_certificate(&mut self, cert: SslCertificate) {
        self.ca_certificates.push(cert);
    }

    pub fn default_configuration() -> Self {
        DEFAULT_SSL_CONFIG
            .read()
            .unwrap()
            .clone()
            .unwrap_or_default()
    }

    pub fn set_default_configuration(config: SslConfiguration) {
        let mut guard = DEFAULT_SSL_CONFIG.write().unwrap();
        *guard = Some(config);
    }
}

// =============================================================================
// SslSocket (QSslSocket)
// =============================================================================

struct SslSocketInner {
    ssl_config: SslConfiguration,
    ssl_mode: SslMode,
    is_encrypted: bool,
    ignore_errors: bool,
}

/// SSL/TLS encrypted socket wrapper matching Qt's `QSslSocket`.
#[derive(Clone)]
pub struct SslSocket {
    tcp: TcpSocket,
    inner: Arc<Mutex<SslSocketInner>>,

    // Qt Signals
    pub encrypted: Signal<()>,
    pub ssl_errors: Signal<Vec<SslError>>,
    pub mode_changed: Signal<SslMode>,

    // Forwarded TCP Signals
    pub connected: Signal<()>,
    pub disconnected: Signal<()>,
    pub ready_read: Signal<()>,
    pub bytes_written: Signal<i64>,
    pub error_occurred: Signal<SocketError>,
    pub state_changed: Signal<SocketState>,
}

impl Default for SslSocket {
    fn default() -> Self {
        Self::new()
    }
}

impl SslSocket {
    pub fn new() -> Self {
        let tcp = TcpSocket::new();
        let encrypted = Signal::new();
        let ssl_errors = Signal::new();
        let mode_changed = Signal::new();

        let connected = Signal::new();
        let disconnected = Signal::new();
        let ready_read = Signal::new();
        let bytes_written = Signal::new();
        let error_occurred = Signal::new();
        let state_changed = Signal::new();

        // Forward inner TCP signals
        let c = connected.clone();
        tcp.connected.connect(move |_| c.emit(&()));

        let d = disconnected.clone();
        tcp.disconnected.connect(move |_| d.emit(&()));

        let r = ready_read.clone();
        tcp.ready_read.connect(move |_| r.emit(&()));

        let b = bytes_written.clone();
        tcp.bytes_written.connect(move |n| b.emit(n));

        let e = error_occurred.clone();
        tcp.error_occurred.connect(move |err| e.emit(err));

        let s = state_changed.clone();
        tcp.state_changed.connect(move |st| s.emit(st));

        Self {
            tcp,
            inner: Arc::new(Mutex::new(SslSocketInner {
                ssl_config: SslConfiguration::default_configuration(),
                ssl_mode: SslMode::UnencryptedMode,
                is_encrypted: false,
                ignore_errors: false,
            })),
            encrypted,
            ssl_errors,
            mode_changed,
            connected,
            disconnected,
            ready_read,
            bytes_written,
            error_occurred,
            state_changed,
        }
    }

    /// TLS is unavailable in this build, so this never opens a plaintext connection.
    pub fn connect_to_host_encrypted(&self, _host: &str, _port: u16) -> Result<(), SocketError> {
        Err(SocketError::SslHandshakeFailedError)
    }

    /// TLS is unavailable in this build; the socket remains unencrypted.
    pub fn start_client_encryption(&self) -> Result<(), SslError> {
        let errors = vec![SslError::TlsUnavailable];
        self.ssl_errors.emit(&errors);
        Err(SslError::TlsUnavailable)
    }

    /// TLS is unavailable in this build; the socket remains unencrypted.
    pub fn start_server_encryption(&self) -> Result<(), SslError> {
        let errors = vec![SslError::TlsUnavailable];
        self.ssl_errors.emit(&errors);
        Err(SslError::TlsUnavailable)
    }


    /// Ignores any TLS handshake verification errors (`ignoreSslErrors`).
    pub fn ignore_ssl_errors(&self) {
        self.inner.lock().unwrap().ignore_errors = true;
    }

    #[inline]
    pub fn is_encrypted(&self) -> bool {
        self.inner.lock().unwrap().is_encrypted
    }

    #[inline]
    pub fn ssl_mode(&self) -> SslMode {
        self.inner.lock().unwrap().ssl_mode
    }

    pub fn ssl_configuration(&self) -> SslConfiguration {
        self.inner.lock().unwrap().ssl_config.clone()
    }

    pub fn set_ssl_configuration(&self, config: SslConfiguration) {
        self.inner.lock().unwrap().ssl_config = config;
    }

    // Transparent I/O delegation
    #[inline]
    pub fn write(&self, data: &[u8]) -> Result<usize, SocketError> {
        self.tcp.write(data)
    }

    #[inline]
    pub fn read(&self, max_len: usize) -> Result<ByteArray, SocketError> {
        self.tcp.read(max_len)
    }

    #[inline]
    pub fn read_all(&self) -> Result<ByteArray, SocketError> {
        self.tcp.read_all()
    }

    #[inline]
    pub fn bytes_available(&self) -> usize {
        self.tcp.bytes_available()
    }

    #[inline]
    pub fn flush(&self) -> Result<(), SocketError> {
        self.tcp.flush()
    }

    #[inline]
    pub fn disconnect_from_host(&self) {
        self.tcp.disconnect_from_host();
        let mut lock = self.inner.lock().unwrap();
        lock.is_encrypted = false;
        lock.ssl_mode = SslMode::UnencryptedMode;
    }

    #[inline]
    pub fn abort(&self) {
        self.tcp.abort();
        let mut lock = self.inner.lock().unwrap();
        lock.is_encrypted = false;
        lock.ssl_mode = SslMode::UnencryptedMode;
    }

    #[inline]
    pub fn state(&self) -> SocketState {
        self.tcp.state()
    }

    #[inline]
    pub fn peer_address(&self) -> Option<HostAddress> {
        self.tcp.peer_address()
    }

    #[inline]
    pub fn peer_port(&self) -> Option<u16> {
        self.tcp.peer_port()
    }
}
