//! Local socket (`QLocalSocket`) and server (`QLocalServer`) for cross-platform IPC.

use std::collections::{HashMap, VecDeque};
use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

use qtrs_core::signal::Signal;
use qtrs_core::types::ByteArray;

use crate::kernel::{HostAddress, SocketError, SocketState};
use crate::tcp::TcpSocket;

// Global IPC registry mapping server names to local bound ports
static LOCAL_SERVER_REGISTRY: RwLock<Option<HashMap<String, u16>>> = RwLock::new(None);

fn register_local_server(name: &str, port: u16) {
    let mut reg = LOCAL_SERVER_REGISTRY.write().unwrap();
    if reg.is_none() {
        *reg = Some(HashMap::new());
    }
    reg.as_mut().unwrap().insert(name.to_string(), port);
}

fn unregister_local_server(name: &str) {
    let mut reg = LOCAL_SERVER_REGISTRY.write().unwrap();
    if let Some(m) = reg.as_mut() {
        m.remove(name);
    }
}

fn lookup_local_server(name: &str) -> Option<u16> {
    let reg = LOCAL_SERVER_REGISTRY.read().unwrap();
    reg.as_ref().and_then(|m| m.get(name).copied())
}

// =============================================================================
// LocalSocket (QLocalSocket)
// =============================================================================

/// Local IPC client socket matching Qt's `QLocalSocket`.
#[derive(Clone)]
pub struct LocalSocket {
    inner: TcpSocket,
    server_name: Arc<Mutex<String>>,

    // Qt Signals
    pub connected: Signal<()>,
    pub disconnected: Signal<()>,
    pub ready_read: Signal<()>,
    pub error_occurred: Signal<SocketError>,
    pub state_changed: Signal<SocketState>,
}

impl Default for LocalSocket {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalSocket {
    pub fn new() -> Self {
        Self::from_tcp(TcpSocket::new())
    }

    /// Constructs from an accepted underlying TCP stream.
    pub fn from_tcp(tcp: TcpSocket) -> Self {
        let connected = Signal::new();
        let disconnected = Signal::new();
        let ready_read = Signal::new();
        let error_occurred = Signal::new();
        let state_changed = Signal::new();

        // Forward inner signals
        let c = connected.clone();
        tcp.connected.connect(move |_| c.emit(&()));

        let d = disconnected.clone();
        tcp.disconnected.connect(move |_| d.emit(&()));

        let r = ready_read.clone();
        tcp.ready_read.connect(move |_| r.emit(&()));

        let e = error_occurred.clone();
        tcp.error_occurred.connect(move |err| e.emit(err));

        let s = state_changed.clone();
        tcp.state_changed.connect(move |st| s.emit(st));

        Self {
            inner: tcp,
            server_name: Arc::new(Mutex::new(String::new())),
            connected,
            disconnected,
            ready_read,
            error_occurred,
            state_changed,
        }
    }

    /// Connects to a named local server (`connectToServer`).
    pub fn connect_to_server(&self, name: &str) -> Result<(), SocketError> {
        let port = lookup_local_server(name).ok_or(SocketError::HostNotFoundError)?;
        *self.server_name.lock().unwrap() = name.to_string();
        self.inner
            .connect_to_address(&HostAddress::from(Ipv4Addr::LOCALHOST), port)
    }

    /// Disconnects from the server cleanly (`disconnectFromServer`).
    pub fn disconnect_from_server(&self) {
        self.inner.disconnect_from_host();
        self.server_name.lock().unwrap().clear();
    }

    /// Writes data to the IPC socket.
    #[inline]
    pub fn write(&self, data: &[u8]) -> Result<usize, SocketError> {
        self.inner.write(data)
    }

    /// Reads up to `max_len` bytes.
    #[inline]
    pub fn read(&self, max_len: usize) -> Result<ByteArray, SocketError> {
        self.inner.read(max_len)
    }

    /// Reads all currently available data.
    #[inline]
    pub fn read_all(&self) -> Result<ByteArray, SocketError> {
        self.inner.read_all()
    }

    /// Flushes outgoing data.
    #[inline]
    pub fn flush(&self) -> Result<(), SocketError> {
        self.inner.flush()
    }

    #[inline]
    pub fn bytes_available(&self) -> usize {
        self.inner.bytes_available()
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    #[inline]
    pub fn state(&self) -> SocketState {
        self.inner.state()
    }

    #[inline]
    pub fn server_name(&self) -> String {
        self.server_name.lock().unwrap().clone()
    }
}

// =============================================================================
// LocalServer (QLocalServer)
// =============================================================================

struct LocalServerInner {
    listener: Option<TcpListener>,
    server_name: String,
    port: Option<u16>,
    pending_connections: VecDeque<LocalSocket>,
    worker_stop: Arc<AtomicBool>,
}

/// Local IPC listening server matching Qt's `QLocalServer`.
#[derive(Clone)]
pub struct LocalServer {
    inner: Arc<Mutex<LocalServerInner>>,

    // Qt Signals
    pub new_connection: Signal<()>,
}

impl Default for LocalServer {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalServer {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(LocalServerInner {
                listener: None,
                server_name: String::new(),
                port: None,
                pending_connections: VecDeque::new(),
                worker_stop: Arc::new(AtomicBool::new(false)),
            })),
            new_connection: Signal::new(),
        }
    }

    /// Listens for IPC connections under the specified server name.
    pub fn listen(&self, name: &str) -> Result<(), SocketError> {
        self.close();

        // Bind to localhost ephemeral port
        let bind_addr = SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
        let listener = TcpListener::bind(bind_addr)?;
        listener.set_nonblocking(true)?;

        let local_port = listener.local_addr()?.port();
        register_local_server(name, local_port);

        {
            let mut lock = self.inner.lock().unwrap();
            lock.listener = Some(listener);
            lock.server_name = name.to_string();
            lock.port = Some(local_port);
        }

        self.start_accept_thread();
        Ok(())
    }

    /// Closes the server and unregisters its name.
    pub fn close(&self) {
        let name = {
            let mut lock = self.inner.lock().unwrap();
            lock.worker_stop.store(true, Ordering::SeqCst);
            lock.listener = None;
            lock.port = None;
            lock.pending_connections.clear();
            std::mem::take(&mut lock.server_name)
        };
        if !name.is_empty() {
            unregister_local_server(&name);
        }
    }

    #[inline]
    pub fn is_listening(&self) -> bool {
        self.inner.lock().unwrap().listener.is_some()
    }

    #[inline]
    pub fn server_name(&self) -> String {
        self.inner.lock().unwrap().server_name.clone()
    }

    pub fn has_pending_connections(&self) -> bool {
        !self.inner.lock().unwrap().pending_connections.is_empty()
    }

    pub fn next_pending_connection(&self) -> Option<LocalSocket> {
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
                        if let Ok(tcp) = TcpSocket::from_stream(stream) {
                            let local_sock = LocalSocket::from_tcp(tcp);
                            {
                                let mut lock = inner_clone.lock().unwrap();
                                lock.pending_connections.push_back(local_sock);
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
