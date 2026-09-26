//! Kernel network primitives and address models matching Qt Network.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use std::str::FromStr;
use std::sync::RwLock;

use qtrs_core::types::ByteArray;

// =============================================================================
// Socket Enums: State & Error
// =============================================================================

/// Socket connection state (`QAbstractSocket::SocketState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SocketState {
    #[default]
    UnconnectedState,
    HostLookupState,
    ConnectingState,
    ConnectedState,
    BoundState,
    ListeningState,
    ClosingState,
}

/// Socket error classification (`QAbstractSocket::SocketError`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SocketError {
    ConnectionRefusedError,
    RemoteHostClosedError,
    HostNotFoundError,
    SocketAccessError,
    SocketResourceError,
    SocketTimeoutError,
    DatagramTooLargeError,
    NetworkError,
    AddressInUseError,
    SocketAddressNotAvailableError,
    UnsupportedSocketOperationError,
    UnfinishedSocketOperationError,
    ProxyAuthenticationRequiredError,
    SslHandshakeFailedError,
    ProxyConnectionRefusedError,
    ProxyConnectionClosedError,
    ProxyConnectionTimeoutError,
    ProxyNotFoundError,
    ProxyProtocolError,
    OperationError,
    SslInternalError,
    SslInvalidUserDataError,
    TemporaryError,
    UnknownSocketError,
}

impl fmt::Display for SocketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for SocketError {}

impl From<std::io::Error> for SocketError {
    fn from(err: std::io::Error) -> Self {
        use std::io::ErrorKind::*;
        match err.kind() {
            ConnectionRefused => SocketError::ConnectionRefusedError,
            ConnectionReset | ConnectionAborted => SocketError::RemoteHostClosedError,
            NotConnected => SocketError::OperationError,
            AddrInUse => SocketError::AddressInUseError,
            AddrNotAvailable => SocketError::SocketAddressNotAvailableError,
            TimedOut => SocketError::SocketTimeoutError,
            PermissionDenied => SocketError::SocketAccessError,
            NotFound => SocketError::HostNotFoundError,
            WouldBlock => SocketError::TemporaryError,
            _ => SocketError::NetworkError,
        }
    }
}

/// Network layer protocol (`QAbstractSocket::NetworkLayerProtocol`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum NetworkLayerProtocol {
    #[default]
    AnyIPProtocol,
    IPv4Protocol,
    IPv6Protocol,
    UnknownNetworkLayerProtocol,
}

// =============================================================================
// SpecialAddress & HostAddress (QHostAddress)
// =============================================================================

/// Standard well-known IP addresses (`QHostAddress::SpecialAddress`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpecialAddress {
    Null,
    Broadcast,
    LocalHost,
    LocalHostIPv6,
    Any,
    AnyIPv6,
    AnyIPv4,
}

/// IP address representation matching `QHostAddress`.
#[derive(Clone, PartialEq, Eq, Hash, Default)]
pub struct HostAddress {
    addr: Option<IpAddr>,
    scope_id: String,
}

impl HostAddress {
    /// Constructs a null address (`QHostAddress::Null`).
    #[inline]
    pub const fn new() -> Self {
        Self {
            addr: None,
            scope_id: String::new(),
        }
    }

    /// Constructs from a standard IPv4 address.
    #[inline]
    pub const fn from_ipv4(ipv4: Ipv4Addr) -> Self {
        Self {
            addr: Some(IpAddr::V4(ipv4)),
            scope_id: String::new(),
        }
    }

    /// Constructs from a standard IPv6 address.
    #[inline]
    pub const fn from_ipv6(ipv6: Ipv6Addr) -> Self {
        Self {
            addr: Some(IpAddr::V6(ipv6)),
            scope_id: String::new(),
        }
    }

    /// Constructs from `SpecialAddress`.
    pub fn from_special(special: SpecialAddress) -> Self {
        match special {
            SpecialAddress::Null => Self::new(),
            SpecialAddress::Broadcast => Self::from_ipv4(Ipv4Addr::BROADCAST),
            SpecialAddress::LocalHost => Self::from_ipv4(Ipv4Addr::LOCALHOST),
            SpecialAddress::LocalHostIPv6 => Self::from_ipv6(Ipv6Addr::LOCALHOST),
            SpecialAddress::Any | SpecialAddress::AnyIPv4 => Self::from_ipv4(Ipv4Addr::UNSPECIFIED),
            SpecialAddress::AnyIPv6 => Self::from_ipv6(Ipv6Addr::UNSPECIFIED),
        }
    }

    /// Returns `true` if this address is null/uninitialized (`isNull()`).
    #[inline]
    pub fn is_null(&self) -> bool {
        self.addr.is_none()
    }

    /// Clears the address, setting it to null.
    #[inline]
    pub fn clear(&mut self) {
        self.addr = None;
        self.scope_id.clear();
    }

    /// Returns the underlying network layer protocol.
    pub fn protocol(&self) -> NetworkLayerProtocol {
        match self.addr {
            Some(IpAddr::V4(_)) => NetworkLayerProtocol::IPv4Protocol,
            Some(IpAddr::V6(_)) => NetworkLayerProtocol::IPv6Protocol,
            None => NetworkLayerProtocol::UnknownNetworkLayerProtocol,
        }
    }

    /// Converts to an IPv4 integer in host-byte order if applicable.
    pub fn to_ipv4_u32(&self) -> Option<u32> {
        match self.addr {
            Some(IpAddr::V4(v4)) => Some(u32::from_be_bytes(v4.octets())),
            _ => None,
        }
    }

    /// Converts to IPv6 16-byte octets if applicable.
    pub fn to_ipv6_octets(&self) -> Option<[u8; 16]> {
        match self.addr {
            Some(IpAddr::V6(v6)) => Some(v6.octets()),
            _ => None,
        }
    }

    /// Converts to standard Rust `IpAddr`.
    #[inline]
    pub fn to_ip_addr(&self) -> Option<IpAddr> {
        self.addr
    }

    /// Converts to standard Rust `SocketAddr` with given port.
    #[inline]
    pub fn to_socket_addr(&self, port: u16) -> Option<SocketAddr> {
        self.addr.map(|ip| SocketAddr::new(ip, port))
    }

    /// Returns the IPv6 scope identifier if set.
    #[inline]
    pub fn scope_id(&self) -> &str {
        &self.scope_id
    }

    /// Sets the IPv6 scope identifier.
    #[inline]
    pub fn set_scope_id(&mut self, scope: impl Into<String>) {
        self.scope_id = scope.into();
    }

    /// Returns `true` if loopback (127.0.0.1 or ::1).
    pub fn is_loopback(&self) -> bool {
        match self.addr {
            Some(IpAddr::V4(v4)) => v4.is_loopback(),
            Some(IpAddr::V6(v6)) => v6.is_loopback(),
            None => false,
        }
    }

    /// Returns `true` if multicast address.
    pub fn is_multicast(&self) -> bool {
        match self.addr {
            Some(IpAddr::V4(v4)) => v4.is_multicast(),
            Some(IpAddr::V6(v6)) => v6.is_multicast(),
            None => false,
        }
    }

    /// Returns `true` if broadcast address (255.255.255.255).
    pub fn is_broadcast(&self) -> bool {
        match self.addr {
            Some(IpAddr::V4(v4)) => v4.is_broadcast(),
            _ => false,
        }
    }

    /// Returns `true` if link-local address.
    pub fn is_link_local(&self) -> bool {
        match self.addr {
            Some(IpAddr::V4(v4)) => v4.is_link_local(),
            Some(IpAddr::V6(v6)) => (v6.segments()[0] & 0xffc0) == 0xfe80,
            None => false,
        }
    }

    /// Returns `true` if private RFC 1918 address (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16).
    pub fn is_private(&self) -> bool {
        match self.addr {
            Some(IpAddr::V4(v4)) => v4.is_private(),
            Some(IpAddr::V6(v6)) => (v6.segments()[0] & 0xfe00) == 0xfc00,
            None => false,
        }
    }

    /// Returns `true` if this address falls within the specified CIDR subnet.
    pub fn is_in_subnet(&self, subnet: &HostAddress, netmask: u8) -> bool {
        match (self.addr, subnet.addr) {
            (Some(IpAddr::V4(ip)), Some(IpAddr::V4(sub))) => {
                if netmask > 32 {
                    return false;
                }
                if netmask == 0 {
                    return true;
                }
                let mask = !((1u32 << (32 - netmask)) - 1);
                let ip_u = u32::from_be_bytes(ip.octets());
                let sub_u = u32::from_be_bytes(sub.octets());
                (ip_u & mask) == (sub_u & mask)
            }
            (Some(IpAddr::V6(ip)), Some(IpAddr::V6(sub))) => {
                if netmask > 128 {
                    return false;
                }
                if netmask == 0 {
                    return true;
                }
                let ip_u = u128::from_be_bytes(ip.octets());
                let sub_u = u128::from_be_bytes(sub.octets());
                let mask = !((1u128 << (128 - netmask)) - 1);
                (ip_u & mask) == (sub_u & mask)
            }
            _ => false,
        }
    }

    /// Parses a CIDR notation subnet string (e.g. `"192.168.1.0/24"`).
    pub fn parse_subnet(subnet_str: &str) -> Option<(Self, u8)> {
        let (ip_part, mask_part) = subnet_str.split_once('/')?;
        let addr = HostAddress::from_str(ip_part).ok()?;
        let mask = mask_part.parse::<u8>().ok()?;
        Some((addr, mask))
    }
}

impl fmt::Display for HostAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.addr {
            Some(ip) => write!(f, "{}", ip),
            None => write!(f, ""),
        }
    }
}

impl fmt::Debug for HostAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HostAddress({})", self)
    }
}

impl FromStr for HostAddress {
    type Err = SocketError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Ok(Self::new());
        }
        match trimmed.parse::<IpAddr>() {
            Ok(ip) => Ok(Self {
                addr: Some(ip),
                scope_id: String::new(),
            }),
            Err(_) => Err(SocketError::HostNotFoundError),
        }
    }
}

impl From<Ipv4Addr> for HostAddress {
    fn from(v4: Ipv4Addr) -> Self {
        Self::from_ipv4(v4)
    }
}

impl From<Ipv6Addr> for HostAddress {
    fn from(v6: Ipv6Addr) -> Self {
        Self::from_ipv6(v6)
    }
}

impl From<IpAddr> for HostAddress {
    fn from(ip: IpAddr) -> Self {
        Self {
            addr: Some(ip),
            scope_id: String::new(),
        }
    }
}

impl From<SpecialAddress> for HostAddress {
    fn from(special: SpecialAddress) -> Self {
        Self::from_special(special)
    }
}

// =============================================================================
// HostInfo (QHostInfo)
// =============================================================================

/// Asynchronous and synchronous DNS host lookup resolver (`QHostInfo`).
#[derive(Debug, Clone, Default)]
pub struct HostInfo {
    host_name: String,
    addresses: Vec<HostAddress>,
    error: Option<SocketError>,
    error_string: String,
}

impl HostInfo {
    /// Performs synchronous DNS resolution for a host name.
    pub fn from_name(name: &str) -> Self {
        let mut info = Self {
            host_name: name.to_string(),
            addresses: Vec::new(),
            error: None,
            error_string: String::new(),
        };

        // Try direct IP parse first
        if let Ok(ip) = name.parse::<IpAddr>() {
            info.addresses.push(HostAddress::from(ip));
            return info;
        }

        // DNS resolution via std ToSocketAddrs
        let lookup_target = format!("{}:0", name);
        match lookup_target.to_socket_addrs() {
            Ok(iter) => {
                for sock in iter {
                    let host_addr = HostAddress::from(sock.ip());
                    if !info.addresses.contains(&host_addr) {
                        info.addresses.push(host_addr);
                    }
                }
                if info.addresses.is_empty() {
                    info.error = Some(SocketError::HostNotFoundError);
                    info.error_string = "Host not found".to_string();
                }
            }
            Err(e) => {
                info.error = Some(SocketError::from(e));
                info.error_string = "DNS resolution failed".to_string();
            }
        }
        info
    }

    /// Resolves a host name directly to addresses or error.
    pub fn lookup_host(name: &str) -> Result<Vec<HostAddress>, SocketError> {
        let info = Self::from_name(name);
        if let Some(err) = info.error {
            Err(err)
        } else {
            Ok(info.addresses)
        }
    }

    pub fn host_name(&self) -> &str {
        &self.host_name
    }

    pub fn addresses(&self) -> &[HostAddress] {
        &self.addresses
    }

    pub fn error(&self) -> Option<SocketError> {
        self.error
    }

    pub fn error_string(&self) -> &str {
        &self.error_string
    }
}

// =============================================================================
// NetworkProxy (QNetworkProxy)
// =============================================================================

/// Network proxy type (`QNetworkProxy::ProxyType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ProxyType {
    #[default]
    DefaultProxy,
    Socks5Proxy,
    NoProxy,
    HttpProxy,
    HttpCachingProxy,
    FtpCachingProxy,
}

/// Network proxy configuration (`QNetworkProxy`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NetworkProxy {
    proxy_type: ProxyType,
    host_name: String,
    port: u16,
    user: String,
    password: String,
}

static APPLICATION_PROXY: RwLock<Option<NetworkProxy>> = RwLock::new(None);

impl NetworkProxy {
    pub fn new(proxy_type: ProxyType, host_name: impl Into<String>, port: u16) -> Self {
        Self {
            proxy_type,
            host_name: host_name.into(),
            port,
            user: String::new(),
            password: String::new(),
        }
    }

    pub fn proxy_type(&self) -> ProxyType {
        self.proxy_type
    }

    pub fn set_type(&mut self, proxy_type: ProxyType) {
        self.proxy_type = proxy_type;
    }

    pub fn host_name(&self) -> &str {
        &self.host_name
    }

    pub fn set_host_name(&mut self, host: impl Into<String>) {
        self.host_name = host.into();
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn set_port(&mut self, port: u16) {
        self.port = port;
    }

    pub fn user(&self) -> &str {
        &self.user
    }

    pub fn set_user(&mut self, user: impl Into<String>) {
        self.user = user.into();
    }

    pub fn password(&self) -> &str {
        &self.password
    }

    pub fn set_password(&mut self, password: impl Into<String>) {
        self.password = password.into();
    }

    /// Returns the global application proxy (`QNetworkProxy::applicationProxy`).
    pub fn application_proxy() -> NetworkProxy {
        APPLICATION_PROXY
            .read()
            .unwrap()
            .clone()
            .unwrap_or_else(|| NetworkProxy {
                proxy_type: ProxyType::NoProxy,
                host_name: String::new(),
                port: 0,
                user: String::new(),
                password: String::new(),
            })
    }

    /// Sets the global application proxy (`QNetworkProxy::setApplicationProxy`).
    pub fn set_application_proxy(proxy: NetworkProxy) {
        let mut guard = APPLICATION_PROXY.write().unwrap();
        *guard = Some(proxy);
    }
}

// =============================================================================
// NetworkDatagram (QNetworkDatagram)
// =============================================================================

/// Independent network datagram with sender & destination metadata (`QNetworkDatagram`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NetworkDatagram {
    data: ByteArray,
    sender_address: HostAddress,
    sender_port: u16,
    destination_address: HostAddress,
    destination_port: u16,
    hop_limit: i32,
    interface_index: u32,
}

impl NetworkDatagram {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_data(data: ByteArray, destination: HostAddress, port: u16) -> Self {
        Self {
            data,
            sender_address: HostAddress::new(),
            sender_port: 0,
            destination_address: destination,
            destination_port: port,
            hop_limit: -1,
            interface_index: 0,
        }
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        !self.data.is_empty()
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        !self.is_valid()
    }

    #[inline]
    pub fn data(&self) -> &ByteArray {
        &self.data
    }

    #[inline]
    pub fn set_data(&mut self, data: ByteArray) {
        self.data = data;
    }

    #[inline]
    pub fn sender_address(&self) -> &HostAddress {
        &self.sender_address
    }

    #[inline]
    pub fn sender_port(&self) -> u16 {
        self.sender_port
    }

    #[inline]
    pub fn set_sender(&mut self, address: HostAddress, port: u16) {
        self.sender_address = address;
        self.sender_port = port;
    }

    #[inline]
    pub fn destination_address(&self) -> &HostAddress {
        &self.destination_address
    }

    #[inline]
    pub fn destination_port(&self) -> u16 {
        self.destination_port
    }

    #[inline]
    pub fn set_destination(&mut self, address: HostAddress, port: u16) {
        self.destination_address = address;
        self.destination_port = port;
    }

    #[inline]
    pub fn hop_limit(&self) -> i32 {
        self.hop_limit
    }

    #[inline]
    pub fn set_hop_limit(&mut self, hop: i32) {
        self.hop_limit = hop;
    }

    #[inline]
    pub fn interface_index(&self) -> u32 {
        self.interface_index
    }

    #[inline]
    pub fn set_interface_index(&mut self, index: u32) {
        self.interface_index = index;
    }

    /// Creates a reply datagram with specified payload addressed to the sender.
    pub fn make_reply(&self, payload: ByteArray) -> Self {
        Self {
            data: payload,
            sender_address: self.destination_address.clone(),
            sender_port: self.destination_port,
            destination_address: self.sender_address.clone(),
            destination_port: self.sender_port,
            hop_limit: self.hop_limit,
            interface_index: self.interface_index,
        }
    }
}
