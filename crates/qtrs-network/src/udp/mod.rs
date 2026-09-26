//! UDP socket implementation (`QUdpSocket`) with datagram queue and multicast support.

use std::collections::VecDeque;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket as StdUdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use qtrs_core::signal::Signal;
use qtrs_core::types::ByteArray;

use crate::kernel::{HostAddress, NetworkDatagram, SocketError, SocketState};

struct UdpSocketInner {
    socket: Option<Arc<StdUdpSocket>>,
    state: SocketState,
    local_address: Option<HostAddress>,
    local_port: Option<u16>,
    rx_queue: VecDeque<NetworkDatagram>,
    worker_stop: Arc<AtomicBool>,
}

/// UDP socket matching Qt's `QUdpSocket`.
#[derive(Clone)]
pub struct UdpSocket {
    inner: Arc<Mutex<UdpSocketInner>>,

    // Qt Signals
    pub ready_read: Signal<()>,
    pub error_occurred: Signal<SocketError>,
    pub state_changed: Signal<SocketState>,
}

impl Default for UdpSocket {
    fn default() -> Self {
        Self::new()
    }
}

impl UdpSocket {
    /// Constructs a new unbound `UdpSocket`.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(UdpSocketInner {
                socket: None,
                state: SocketState::UnconnectedState,
                local_address: None,
                local_port: None,
                rx_queue: VecDeque::new(),
                worker_stop: Arc::new(AtomicBool::new(false)),
            })),
            ready_read: Signal::new(),
            error_occurred: Signal::new(),
            state_changed: Signal::new(),
        }
    }

    /// Binds the socket to `address` and `port`.
    pub fn bind(&self, address: &HostAddress, port: u16) -> Result<(), SocketError> {
        self.close();

        let ip = address
            .to_ip_addr()
            .unwrap_or(std::net::IpAddr::V4(Ipv4Addr::UNSPECIFIED));
        let bind_addr = SocketAddr::new(ip, port);

        let sock = StdUdpSocket::bind(bind_addr)?;
        sock.set_nonblocking(true)?;

        let local = sock.local_addr()?;
        let sock_arc = Arc::new(sock);

        {
            let mut lock = self.inner.lock().unwrap();
            lock.local_address = Some(HostAddress::from(local.ip()));
            lock.local_port = Some(local.port());
            lock.socket = Some(Arc::clone(&sock_arc));
            lock.state = SocketState::BoundState;
        }

        self.start_rx_thread(sock_arc);
        self.state_changed.emit(&SocketState::BoundState);
        Ok(())
    }

    /// Closes the UDP socket.
    pub fn close(&self) {
        let mut lock = self.inner.lock().unwrap();
        lock.worker_stop.store(true, Ordering::SeqCst);
        lock.socket = None;
        lock.local_address = None;
        lock.local_port = None;
        lock.rx_queue.clear();
        lock.state = SocketState::UnconnectedState;
        drop(lock);
        self.state_changed.emit(&SocketState::UnconnectedState);
    }

    /// Returns `true` if one or more datagrams are waiting to be read.
    pub fn has_pending_datagrams(&self) -> bool {
        !self.inner.lock().unwrap().rx_queue.is_empty()
    }

    /// Returns the size in bytes of the next pending datagram.
    pub fn pending_datagram_size(&self) -> Option<usize> {
        self.inner.lock().unwrap().rx_queue.front().map(|d| d.data().len())
    }

    /// Reads the next pending `NetworkDatagram`.
    pub fn read_datagram(&self) -> Option<NetworkDatagram> {
        self.inner.lock().unwrap().rx_queue.pop_front()
    }

    /// Writes a datagram to its designated recipient.
    pub fn write_datagram(&self, datagram: &NetworkDatagram) -> Result<usize, SocketError> {
        let dest_ip = datagram
            .destination_address()
            .to_ip_addr()
            .ok_or(SocketError::SocketAddressNotAvailableError)?;
        let dest_addr = SocketAddr::new(dest_ip, datagram.destination_port());

        self.send_to_addr(datagram.data().as_bytes(), dest_addr)
    }

    /// Sends raw bytes to a specific destination host and port.
    pub fn send_to(&self, data: &[u8], host: &HostAddress, port: u16) -> Result<usize, SocketError> {
        let dest_ip = host
            .to_ip_addr()
            .ok_or(SocketError::SocketAddressNotAvailableError)?;
        let dest_addr = SocketAddr::new(dest_ip, port);
        self.send_to_addr(data, dest_addr)
    }

    fn send_to_addr(&self, data: &[u8], addr: SocketAddr) -> Result<usize, SocketError> {
        let sock_opt = self.inner.lock().unwrap().socket.clone();
        let sock = match sock_opt {
            Some(s) => s,
            None => {
                self.bind(&HostAddress::new(), 0)?;
                self.inner.lock().unwrap().socket.clone().ok_or(SocketError::OperationError)?
            }
        };

        let sent = sock.send_to(data, addr)?;
        Ok(sent)
    }

    /// Joins an IPv4 multicast group.
    pub fn join_multicast_group(&self, group: &HostAddress) -> Result<(), SocketError> {
        let lock = self.inner.lock().unwrap();
        let sock = lock.socket.as_ref().ok_or(SocketError::OperationError)?;
        match group.to_ip_addr() {
            Some(std::net::IpAddr::V4(v4)) => {
                sock.join_multicast_v4(&v4, &Ipv4Addr::UNSPECIFIED)?;
                Ok(())
            }
            Some(std::net::IpAddr::V6(v6)) => {
                sock.join_multicast_v6(&v6, 0)?;
                Ok(())
            }
            None => Err(SocketError::SocketAddressNotAvailableError),
        }
    }

    /// Leaves an IPv4 multicast group.
    pub fn leave_multicast_group(&self, group: &HostAddress) -> Result<(), SocketError> {
        let lock = self.inner.lock().unwrap();
        let sock = lock.socket.as_ref().ok_or(SocketError::OperationError)?;
        match group.to_ip_addr() {
            Some(std::net::IpAddr::V4(v4)) => {
                sock.leave_multicast_v4(&v4, &Ipv4Addr::UNSPECIFIED)?;
                Ok(())
            }
            Some(std::net::IpAddr::V6(v6)) => {
                sock.leave_multicast_v6(&v6, 0)?;
                Ok(())
            }
            None => Err(SocketError::SocketAddressNotAvailableError),
        }
    }

    pub fn state(&self) -> SocketState {
        self.inner.lock().unwrap().state
    }

    pub fn local_address(&self) -> Option<HostAddress> {
        self.inner.lock().unwrap().local_address.clone()
    }

    pub fn local_port(&self) -> Option<u16> {
        self.inner.lock().unwrap().local_port
    }

    fn start_rx_thread(&self, sock: Arc<StdUdpSocket>) {
        let stop_flag = {
            let mut lock = self.inner.lock().unwrap();
            lock.worker_stop.store(true, Ordering::SeqCst);
            lock.worker_stop = Arc::new(AtomicBool::new(false));
            Arc::clone(&lock.worker_stop)
        };

        let inner_clone = Arc::clone(&self.inner);
        let ready_sig = self.ready_read.clone();

        thread::spawn(move || {
            let mut buf = [0u8; 65536]; // Max UDP packet size

            while !stop_flag.load(Ordering::Relaxed) {
                match sock.recv_from(&mut buf) {
                    Ok((n, src_addr)) => {
                        let mut datagram = NetworkDatagram::new();
                        datagram.set_data(ByteArray::from(&buf[..n]));
                        datagram.set_sender(HostAddress::from(src_addr.ip()), src_addr.port());

                        if let Ok(loc) = sock.local_addr() {
                            datagram.set_destination(HostAddress::from(loc.ip()), loc.port());
                        }

                        {
                            let mut lock = inner_clone.lock().unwrap();
                            lock.rx_queue.push_back(datagram);
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
