//! TCP client socket (`QTcpSocket`) and listening server (`QTcpServer`).

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use qtrs_core::signal::Signal;
use qtrs_core::types::ByteArray;

use crate::kernel::{HostAddress, SocketError, SocketState};

// =============================================================================
// TcpSocket (QTcpSocket)
// =============================================================================

struct TcpSocketInner {
    stream: Option<TcpStream>,
    state: SocketState,
    error: Option<SocketError>,
    error_string: String,
    peer_address: Option<HostAddress>,
    peer_port: Option<u16>,
    local_address: Option<HostAddress>,
    local_port: Option<u16>,
    rx_buffer: VecDeque<u8>,
    worker_stop: Arc<AtomicBool>,
}

/// TCP client socket implementation matching Qt's `QTcpSocket`.
#[derive(Clone)]
pub struct TcpSocket {
    inner: Arc<Mutex<TcpSocketInner>>,

    // Qt Signals
    pub connected: Signal<()>,
    pub disconnected: Signal<()>,
    pub ready_read: Signal<()>,
    pub bytes_written: Signal<i64>,
    pub error_occurred: Signal<SocketError>,
    pub state_changed: Signal<SocketState>,
}

impl Default for TcpSocket {
    fn default() -> Self {
        Self::new()
    }
}

impl TcpSocket {
    /// Constructs a new unconnected `TcpSocket`.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(TcpSocketInner {
                stream: None,
                state: SocketState::UnconnectedState,
                error: None,
                error_string: String::new(),
                peer_address: None,
                peer_port: None,
                local_address: None,
                local_port: None,
                rx_buffer: VecDeque::new(),
                worker_stop: Arc::new(AtomicBool::new(false)),
            })),
            connected: Signal::new(),
            disconnected: Signal::new(),
            ready_read: Signal::new(),
            bytes_written: Signal::new(),
            error_occurred: Signal::new(),
            state_changed: Signal::new(),
        }
    }

    /// Constructs a connected `TcpSocket` wrapping an active `TcpStream`.
    pub fn from_stream(stream: TcpStream) -> Result<Self, SocketError> {
        let sock = Self::new();
        sock.init_with_stream(stream)?;
        Ok(sock)
    }

    fn init_with_stream(&self, stream: TcpStream) -> Result<(), SocketError> {
        stream.set_nonblocking(true)?;
        stream.set_nodelay(true)?;

        let peer = stream.peer_addr().ok().map(|p| (HostAddress::from(p.ip()), p.port()));
        let local = stream.local_addr().ok().map(|l| (HostAddress::from(l.ip()), l.port()));

        {
            let mut lock = self.inner.lock().unwrap();
            lock.stream = Some(stream);
            if let Some((addr, port)) = peer {
                lock.peer_address = Some(addr);
                lock.peer_port = Some(port);
            }
            if let Some((addr, port)) = local {
                lock.local_address = Some(addr);
                lock.local_port = Some(port);
            }
            lock.state = SocketState::ConnectedState;
        }

        self.start_reader_thread();
        self.state_changed.emit(&SocketState::ConnectedState);
        Ok(())
    }

    /// Connects to a remote host specified by name and port.
    pub fn connect_to_host(&self, host_name: &str, port: u16) -> Result<(), SocketError> {
        self.abort();
        self.set_state(SocketState::HostLookupState);

        let target = format!("{}:{}", host_name, port);
        let mut addrs = match target.to_socket_addrs() {
            Ok(iter) => iter,
            Err(e) => {
                let err = SocketError::from(e);
                self.set_error(err, "Host lookup failed");
                return Err(err);
            }
        };

        let addr = addrs.next().ok_or(SocketError::HostNotFoundError)?;
        self.set_state(SocketState::ConnectingState);

        let stream = match TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
            Ok(s) => s,
            Err(e) => {
                let err = SocketError::from(e);
                self.set_error(err, "Connection refused or timed out");
                return Err(err);
            }
        };

        self.init_with_stream(stream)?;
        self.connected.emit(&());
        Ok(())
    }

    /// Connects to an explicit `HostAddress`.
    pub fn connect_to_address(&self, address: &HostAddress, port: u16) -> Result<(), SocketError> {
        let ip = address.to_ip_addr().ok_or(SocketError::SocketAddressNotAvailableError)?;
        let addr = SocketAddr::new(ip, port);

        self.abort();
        self.set_state(SocketState::ConnectingState);

        let stream = match TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
            Ok(s) => s,
            Err(e) => {
                let err = SocketError::from(e);
                self.set_error(err, "Connection refused");
                return Err(err);
            }
        };

        self.init_with_stream(stream)?;
        self.connected.emit(&());
        Ok(())
    }

    /// Disconnects from host cleanly.
    pub fn disconnect_from_host(&self) {
        {
            let mut lock = self.inner.lock().unwrap();
            if lock.state == SocketState::UnconnectedState {
                return;
            }
            lock.state = SocketState::ClosingState;
            if let Some(stream) = &lock.stream {
                let _ = stream.shutdown(Shutdown::Both);
            }
            lock.worker_stop.store(true, Ordering::SeqCst);
            lock.stream = None;
            lock.state = SocketState::UnconnectedState;
        }
        self.state_changed.emit(&SocketState::UnconnectedState);
        self.disconnected.emit(&());
    }

    /// Aborts current connection immediately and resets the socket.
    pub fn abort(&self) {
        self.disconnect_from_host();
        let mut lock = self.inner.lock().unwrap();
        lock.rx_buffer.clear();
        lock.error = None;
        lock.error_string.clear();
    }

    /// Writes binary data into the TCP stream.
    pub fn write(&self, data: &[u8]) -> Result<usize, SocketError> {
        let mut lock = self.inner.lock().unwrap();
        let stream = lock.stream.as_mut().ok_or(SocketError::OperationError)?;
        match stream.write(data) {
            Ok(written) => {
                drop(lock);
                self.bytes_written.emit(&(written as i64));
                Ok(written)
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
            Err(e) => {
                let err = SocketError::from(e);
                drop(lock);
                self.set_error(err, "Write failed");
                Err(err)
            }
        }
    }

    /// Reads up to `max_len` bytes from internal buffer.
    pub fn read(&self, max_len: usize) -> Result<ByteArray, SocketError> {
        let mut lock = self.inner.lock().unwrap();
        let count = lock.rx_buffer.len().min(max_len);
        let mut data = Vec::with_capacity(count);
        for _ in 0..count {
            if let Some(b) = lock.rx_buffer.pop_front() {
                data.push(b);
            }
        }
        Ok(ByteArray::from(data))
    }

    /// Reads all currently available bytes from the buffer.
    pub fn read_all(&self) -> Result<ByteArray, SocketError> {
        let mut lock = self.inner.lock().unwrap();
        let data: Vec<u8> = lock.rx_buffer.drain(..).collect();
        Ok(ByteArray::from(data))
    }

    /// Returns the number of bytes available for reading.
    pub fn bytes_available(&self) -> usize {
        self.inner.lock().unwrap().rx_buffer.len()
    }

    /// Flushes outgoing data.
    pub fn flush(&self) -> Result<(), SocketError> {
        let mut lock = self.inner.lock().unwrap();
        if let Some(stream) = lock.stream.as_mut() {
            stream.flush()?;
        }
        Ok(())
    }

    pub fn state(&self) -> SocketState {
        self.inner.lock().unwrap().state
    }

    pub fn is_valid(&self) -> bool {
        self.state() == SocketState::ConnectedState
    }

    pub fn error(&self) -> Option<SocketError> {
        self.inner.lock().unwrap().error
    }

    pub fn error_string(&self) -> String {
        self.inner.lock().unwrap().error_string.clone()
    }

    pub fn peer_address(&self) -> Option<HostAddress> {
        self.inner.lock().unwrap().peer_address.clone()
    }

    pub fn peer_port(&self) -> Option<u16> {
        self.inner.lock().unwrap().peer_port
    }

    pub fn local_address(&self) -> Option<HostAddress> {
        self.inner.lock().unwrap().local_address.clone()
    }

    pub fn local_port(&self) -> Option<u16> {
        self.inner.lock().unwrap().local_port
    }

    fn set_state(&self, new_state: SocketState) {
        let mut lock = self.inner.lock().unwrap();
        if lock.state != new_state {
            lock.state = new_state;
            drop(lock);
            self.state_changed.emit(&new_state);
        }
    }

    fn set_error(&self, err: SocketError, msg: &str) {
        let mut lock = self.inner.lock().unwrap();
        lock.error = Some(err);
        lock.error_string = msg.to_string();
        drop(lock);
        self.error_occurred.emit(&err);
    }

    fn start_reader_thread(&self) {
        let (stream_clone, stop_flag) = {
            let mut lock = self.inner.lock().unwrap();
            lock.worker_stop.store(true, Ordering::SeqCst);
            lock.worker_stop = Arc::new(AtomicBool::new(false));

            let s = match lock.stream.as_ref() {
                Some(st) => match st.try_clone() {
                    Ok(c) => c,
                    Err(_) => return,
                },
                None => return,
            };
            (s, Arc::clone(&lock.worker_stop))
        };

        let inner_clone = Arc::clone(&self.inner);
        let ready_sig = self.ready_read.clone();

        thread::spawn(move || {
            let mut read_stream = stream_clone;
            let mut temp_buf = [0u8; 8192];

            while !stop_flag.load(Ordering::Relaxed) {
                match read_stream.read(&mut temp_buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        {
                            let mut lock = inner_clone.lock().unwrap();
                            lock.rx_buffer.extend(&temp_buf[..n]);
                        }
                        ready_sig.emit(&());
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
    }
}

// =============================================================================
// TcpServer (QTcpServer)
// =============================================================================

struct TcpServerInner {
    listener: Option<TcpListener>,
    server_address: Option<HostAddress>,
    server_port: Option<u16>,
    pending_connections: VecDeque<TcpSocket>,
    worker_stop: Arc<AtomicBool>,
}

/// TCP listening server implementation matching Qt's `QTcpServer`.
#[derive(Clone)]
pub struct TcpServer {
    inner: Arc<Mutex<TcpServerInner>>,

    // Qt Signals
    pub new_connection: Signal<()>,
}

impl Default for TcpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl TcpServer {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(TcpServerInner {
                listener: None,
                server_address: None,
                server_port: None,
                pending_connections: VecDeque::new(),
                worker_stop: Arc::new(AtomicBool::new(false)),
            })),
            new_connection: Signal::new(),
        }
    }

    /// Listens for incoming connections on `address` and `port`.
    pub fn listen(&self, address: &HostAddress, port: u16) -> Result<(), SocketError> {
        self.close();

        let ip = address
            .to_ip_addr()
            .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));
        let bind_addr = SocketAddr::new(ip, port);

        let listener = TcpListener::bind(bind_addr)?;
        listener.set_nonblocking(true)?;

        let local_addr = listener.local_addr()?;
        {
            let mut lock = self.inner.lock().unwrap();
            lock.server_address = Some(HostAddress::from(local_addr.ip()));
            lock.server_port = Some(local_addr.port());
            lock.listener = Some(listener);
        }

        self.start_accept_thread();
        Ok(())
    }

    /// Closes the server.
    pub fn close(&self) {
        let mut lock = self.inner.lock().unwrap();
        lock.worker_stop.store(true, Ordering::SeqCst);
        lock.listener = None;
        lock.server_address = None;
        lock.server_port = None;
        lock.pending_connections.clear();
    }

    /// Returns `true` if the server is currently listening.
    pub fn is_listening(&self) -> bool {
        self.inner.lock().unwrap().listener.is_some()
    }

    pub fn server_address(&self) -> Option<HostAddress> {
        self.inner.lock().unwrap().server_address.clone()
    }

    pub fn server_port(&self) -> Option<u16> {
        self.inner.lock().unwrap().server_port
    }

    /// Returns `true` if incoming connections are pending.
    pub fn has_pending_connections(&self) -> bool {
        !self.inner.lock().unwrap().pending_connections.is_empty()
    }

    /// Retrieves the next pending connection as a `TcpSocket`.
    pub fn next_pending_connection(&self) -> Option<TcpSocket> {
        self.inner.lock().unwrap().pending_connections.pop_front()
    }

    fn start_accept_thread(&self) {
        let (listener, stop_flag) = {
            let mut lock = self.inner.lock().unwrap();
            lock.worker_stop.store(true, Ordering::SeqCst);
            lock.worker_stop = Arc::new(AtomicBool::new(false));

            let l = match lock.listener.as_ref() {
                Some(lst) => match lst.try_clone() {
                    Ok(c) => c,
                    Err(_) => return,
                },
                None => return,
            };
            (l, Arc::clone(&lock.worker_stop))
        };

        let inner_clone = Arc::clone(&self.inner);
        let new_conn_sig = self.new_connection.clone();

        thread::spawn(move || {
            while !stop_flag.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        if let Ok(socket) = TcpSocket::from_stream(stream) {
                            {
                                let mut lock = inner_clone.lock().unwrap();
                                lock.pending_connections.push_back(socket);
                            }
                            new_conn_sig.emit(&());
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });
    }
}
