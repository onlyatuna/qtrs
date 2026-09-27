use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::event::{NativeEventFilter, NativeEventFilterChain, NativeMessage};
use crate::event_loop::dispatcher::{DispatchResult, EventDispatcher, EventDispatcherHandle};
use crate::event_loop::socket_notifier::{SocketDescriptor, SocketEvent, SocketNotifier};
use crate::timer::{TimerEntry, TimerId, TimerRegistry};

pub const EPOLLIN: u32 = 0x001;
pub const EPOLLPRI: u32 = 0x002;
pub const EPOLLOUT: u32 = 0x004;
pub const EPOLLERR: u32 = 0x008;
pub const EPOLLHUP: u32 = 0x010;

pub const EPOLL_CTL_ADD: i32 = 1;
pub const EPOLL_CTL_DEL: i32 = 2;
pub const EPOLL_CTL_MOD: i32 = 3;

/// Cross-thread wakeup primitive backing [`EpollReactor`].
///
/// On Linux this wraps a real `eventfd(2)` registered into the reactor's epoll instance, so a
/// `wake_up()` from another thread is observed by the kernel `epoll_wait(2)` call itself instead
/// of only a same-process `Condvar` that nobody may be parked on at that instant (the previous,
/// purely in-process `AtomicU64` simulation could not be seen by `epoll_wait`, so a wakeup that
/// landed while this thread was blocked inside that syscall was silently lost until the full
/// timeout elapsed). Non-Linux Unix targets, which never create a real epoll instance here, keep
/// the atomic counter + condvar as their only mechanism.
#[derive(Debug)]
pub struct EventFd {
    #[cfg(target_os = "linux")]
    fd: std::os::raw::c_int,
    #[cfg(not(target_os = "linux"))]
    counter: AtomicU64,
}

impl EventFd {
    #[cfg(target_os = "linux")]
    pub fn new(init: u64) -> Self {
        // SAFETY: eventfd(2) with a plain integer init value and no pointers; the returned fd is
        // owned exclusively by this EventFd until Drop closes it.
        let fd = unsafe {
            linux_epoll::eventfd(
                init as u32,
                linux_epoll::EFD_NONBLOCK | linux_epoll::EFD_CLOEXEC,
            )
        };
        Self { fd }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn new(init: u64) -> Self {
        Self {
            counter: AtomicU64::new(init),
        }
    }

    /// The raw eventfd descriptor, for registering with `epoll_ctl`. `None` if creation failed
    /// or this is not the Linux backend.
    #[cfg(target_os = "linux")]
    pub fn raw_fd(&self) -> Option<std::os::raw::c_int> {
        (self.fd >= 0).then_some(self.fd)
    }

    #[cfg(not(target_os = "linux"))]
    pub fn raw_fd(&self) -> Option<std::os::raw::c_int> {
        None
    }

    #[cfg(target_os = "linux")]
    pub fn write(&self, val: u64) {
        if self.fd < 0 {
            return;
        }
        let buf = val.to_ne_bytes();
        loop {
            // SAFETY: fd is a valid, owned eventfd; buf is a live 8-byte buffer for the duration
            // of the call.
            let ret = unsafe {
                linux_epoll::write(
                    self.fd,
                    buf.as_ptr() as *const std::os::raw::c_void,
                    buf.len(),
                )
            };
            if ret >= 0 {
                return;
            }
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            // EAGAIN here would mean the counter is already at u64::MAX-1 pending wakeups; there
            // is no useful synchronous retry, and one already-pending wakeup is all a waiter
            // needs, so it's safe to drop.
            return;
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn write(&self, val: u64) {
        self.counter.fetch_add(val, Ordering::SeqCst);
    }

    /// Drains the counter, returning the total value read (0 if nothing was pending). On Linux
    /// this fully empties the eventfd (looping until `EAGAIN`) so `epoll_wait` won't spuriously
    /// report it readable again on the next call.
    #[cfg(target_os = "linux")]
    pub fn read(&self) -> u64 {
        if self.fd < 0 {
            return 0;
        }
        let mut total = 0u64;
        loop {
            let mut buf = [0u8; 8];
            // SAFETY: fd is a valid, owned eventfd; buf is a live 8-byte buffer for the duration
            // of the call.
            let ret = unsafe {
                linux_epoll::read(
                    self.fd,
                    buf.as_mut_ptr() as *mut std::os::raw::c_void,
                    buf.len(),
                )
            };
            if ret == 8 {
                total = total.wrapping_add(u64::from_ne_bytes(buf));
                continue;
            }
            if ret < 0 {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
            }
            // EAGAIN (empty, EFD_NONBLOCK) or a short/zero read: fully drained.
            return total;
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn read(&self) -> u64 {
        self.counter.swap(0, Ordering::SeqCst)
    }
}

#[cfg(target_os = "linux")]
impl Drop for EventFd {
    fn drop(&mut self) {
        if self.fd >= 0 {
            unsafe {
                linux_epoll::close(self.fd);
            }
        }
    }
}

/// Simulated Linux `timerfd` structure
#[derive(Debug)]
pub struct TimerFd {
    pub id: TimerId,
    pub interval: Duration,
    pub single_shot: bool,
    pub next_fire: Mutex<Option<Instant>>,
    pub expirations: AtomicU64,
}

impl TimerFd {
    pub fn new(id: TimerId, interval: Duration, single_shot: bool) -> Self {
        let next_fire = Instant::now() + interval;
        Self {
            id,
            interval,
            single_shot,
            next_fire: Mutex::new(Some(next_fire)),
            expirations: AtomicU64::new(0),
        }
    }

    pub fn check_and_fire(&self, now: Instant) -> bool {
        let mut next = self.next_fire.lock().unwrap();
        if let Some(target) = *next {
            if now >= target {
                self.expirations.fetch_add(1, Ordering::SeqCst);
                if self.single_shot {
                    *next = None;
                } else {
                    *next = Some(now + self.interval);
                }
                return true;
            }
        }
        false
    }

    pub fn read_expirations(&self) -> u64 {
        self.expirations.swap(0, Ordering::SeqCst)
    }
}

#[cfg(target_os = "linux")]
mod linux_epoll {
    use std::os::raw::{c_int, c_void};

    pub const EPOLLIN: u32 = 0x001;
    pub const EPOLLPRI: u32 = 0x002;
    pub const EPOLLOUT: u32 = 0x004;
    pub const EPOLLERR: u32 = 0x008;
    pub const EPOLLHUP: u32 = 0x010;
    pub const EPOLL_CLOEXEC: c_int = 0x80000;

    pub const EPOLL_CTL_ADD: c_int = 1;
    pub const EPOLL_CTL_DEL: c_int = 2;
    #[allow(dead_code)] // part of the epoll_ctl API surface; no caller needs EPOLL_CTL_MOD yet
    pub const EPOLL_CTL_MOD: c_int = 3;

    // octal 04000 / 02000000 per Linux's <asm-generic/fcntl.h> O_NONBLOCK/O_CLOEXEC, which
    // eventfd(2) reuses for EFD_NONBLOCK/EFD_CLOEXEC.
    pub const EFD_NONBLOCK: c_int = 0o4000;
    pub const EFD_CLOEXEC: c_int = 0o2000000;

    #[repr(C)]
    #[cfg_attr(target_arch = "x86_64", repr(packed))]
    #[derive(Debug, Copy, Clone)]
    pub struct EpollEvent {
        pub events: u32,
        pub data: u64,
    }

    extern "C" {
        pub fn epoll_create1(flags: c_int) -> c_int;
        pub fn epoll_ctl(epfd: c_int, op: c_int, fd: c_int, event: *mut EpollEvent) -> c_int;
        pub fn epoll_wait(
            epfd: c_int,
            events: *mut EpollEvent,
            maxevents: c_int,
            timeout: c_int,
        ) -> c_int;
        pub fn close(fd: c_int) -> c_int;
        pub fn eventfd(initval: u32, flags: c_int) -> c_int;
        pub fn read(fd: c_int, buf: *mut c_void, count: usize) -> isize;
        pub fn write(fd: c_int, buf: *const c_void, count: usize) -> isize;
    }
}

pub struct EpollReactor {
    pub event_fd: Arc<EventFd>,
    pub timer_fds: Mutex<HashMap<TimerId, Arc<TimerFd>>>,
    pub socket_notifiers: Mutex<HashMap<(SocketDescriptor, SocketEvent), Arc<SocketNotifier>>>,
    pub pending_socket_events: Mutex<Vec<(SocketDescriptor, SocketEvent)>>,
    cond: Condvar,
    lock: Mutex<bool>,
    #[cfg(target_os = "linux")]
    epoll_fd: std::os::raw::c_int,
}

impl Default for EpollReactor {
    fn default() -> Self {
        Self::new()
    }
}

impl EpollReactor {
    pub fn new() -> Self {
        #[cfg(target_os = "linux")]
        let epoll_fd = unsafe { linux_epoll::epoll_create1(linux_epoll::EPOLL_CLOEXEC) };

        let event_fd = Arc::new(EventFd::new(0));

        // Register the wakeup eventfd with epoll immediately at construction, not "after the
        // fact" from some other call site: this is what lets a real epoll_wait(2) observe a
        // cross-thread wake_up() while blocked, instead of only a Condvar nobody may be parked
        // on at that instant.
        #[cfg(target_os = "linux")]
        if epoll_fd >= 0 {
            if let Some(wakeup_fd) = event_fd.raw_fd() {
                let mut ev = linux_epoll::EpollEvent {
                    events: linux_epoll::EPOLLIN,
                    data: wakeup_fd as u64,
                };
                unsafe {
                    linux_epoll::epoll_ctl(
                        epoll_fd,
                        linux_epoll::EPOLL_CTL_ADD,
                        wakeup_fd,
                        &mut ev,
                    );
                }
            }
        }

        Self {
            event_fd,
            timer_fds: Mutex::new(HashMap::new()),
            socket_notifiers: Mutex::new(HashMap::new()),
            pending_socket_events: Mutex::new(Vec::new()),
            cond: Condvar::new(),
            lock: Mutex::new(false),
            #[cfg(target_os = "linux")]
            epoll_fd,
        }
    }

    pub fn wake_up(&self) {
        self.event_fd.write(1);
        let _guard = self.lock.lock().unwrap();
        self.cond.notify_all();
    }

    pub fn register_timer(&self, id: TimerId, interval: Duration, single_shot: bool) {
        let tfd = Arc::new(TimerFd::new(id, interval, single_shot));
        self.timer_fds.lock().unwrap().insert(id, tfd);
        let _guard = self.lock.lock().unwrap();
        self.cond.notify_all();
    }

    pub fn unregister_timer(&self, id: TimerId) {
        self.timer_fds.lock().unwrap().remove(&id);
    }

    pub fn register_socket_notifier(&self, notifier: &Arc<SocketNotifier>) {
        let key = (notifier.descriptor(), notifier.event_type());
        self.socket_notifiers
            .lock()
            .unwrap()
            .insert(key, Arc::clone(notifier));
        #[cfg(target_os = "linux")]
        if self.epoll_fd >= 0 {
            let mut ev = linux_epoll::EpollEvent {
                events: match notifier.event_type() {
                    SocketEvent::Read => {
                        linux_epoll::EPOLLIN | linux_epoll::EPOLLERR | linux_epoll::EPOLLHUP
                    }
                    SocketEvent::Write => {
                        linux_epoll::EPOLLOUT | linux_epoll::EPOLLERR | linux_epoll::EPOLLHUP
                    }
                    SocketEvent::Exception => {
                        linux_epoll::EPOLLPRI | linux_epoll::EPOLLERR | linux_epoll::EPOLLHUP
                    }
                },
                data: notifier.descriptor() as u64,
            };
            unsafe {
                linux_epoll::epoll_ctl(
                    self.epoll_fd,
                    linux_epoll::EPOLL_CTL_ADD,
                    notifier.descriptor() as std::os::raw::c_int,
                    &mut ev,
                );
            }
        }

        let _guard = self.lock.lock().unwrap();
        self.cond.notify_all();
    }

    pub fn unregister_socket_notifier(&self, notifier: &Arc<SocketNotifier>) {
        let key = (notifier.descriptor(), notifier.event_type());
        self.socket_notifiers.lock().unwrap().remove(&key);

        #[cfg(target_os = "linux")]
        if self.epoll_fd >= 0 {
            unsafe {
                linux_epoll::epoll_ctl(
                    self.epoll_fd,
                    linux_epoll::EPOLL_CTL_DEL,
                    notifier.descriptor() as std::os::raw::c_int,
                    std::ptr::null_mut(),
                );
            }
        }
    }

    pub fn trigger_socket_event(&self, fd: SocketDescriptor, event: SocketEvent) {
        self.pending_socket_events.lock().unwrap().push((fd, event));
        let _guard = self.lock.lock().unwrap();
        self.cond.notify_all();
    }

    pub fn epoll_wait(
        &self,
        timeout: Option<Duration>,
    ) -> (bool, Vec<TimerId>, Vec<(SocketDescriptor, SocketEvent)>) {
        let start = Instant::now();
        let guard = self.lock.lock().unwrap();
        let pending_sockets = std::mem::take(&mut *self.pending_socket_events.lock().unwrap());

        // Drain the wakeup eventfd first: catches a wake_up() that landed before this call even
        // began (e.g. between the caller computing its timeout and taking `self.lock` above).
        if self.event_fd.read() > 0 {
            return (true, self.collect_expired_timers(start), pending_sockets);
        }

        // Return immediately if there are pending socket events
        if !pending_sockets.is_empty() {
            return (false, self.collect_expired_timers(start), pending_sockets);
        }

        // Check if any timers expired
        let expired = self.collect_expired_timers(start);
        if !expired.is_empty() {
            return (false, expired, Vec::new());
        }
        // Calculate sleep timeout
        let sleep_duration = match timeout {
            Some(t) => {
                let earliest_timer = self.next_timer_delay(start);
                match earliest_timer {
                    Some(td) => t.min(td),
                    None => t,
                }
            }
            None => self
                .next_timer_delay(start)
                .unwrap_or(Duration::from_secs(3600)),
        };

        if sleep_duration.is_zero() {
            return (false, Vec::new(), Vec::new());
        }
        #[cfg(target_os = "linux")]
        if self.epoll_fd >= 0 {
            let timeout_ms = match sleep_duration {
                d if d.is_zero() => 0,
                d => (d.as_millis() as std::os::raw::c_int).min(i32::MAX as std::os::raw::c_int),
            };

            let mut events = [linux_epoll::EpollEvent { events: 0, data: 0 }; 64];
            // This is the real OS-level wakeup: the wakeup eventfd was registered with this
            // epoll instance in EpollReactor::new(), so a wake_up() on another thread that
            // writes it is observed here directly by the kernel — not just by a Condvar that
            // may have nobody parked on it while this call blocks.
            let nfds = unsafe {
                linux_epoll::epoll_wait(self.epoll_fd, events.as_mut_ptr(), 64, timeout_ms)
            };

            let mut awoken = false;
            let mut kernel_sockets = Vec::new();
            if nfds > 0 {
                let wakeup_fd = self.event_fd.raw_fd();
                for &ev in events.iter().take(nfds as usize) {
                    let fd = ev.data as std::os::raw::c_int;
                    if wakeup_fd == Some(fd) {
                        // EFD_NONBLOCK: drain fully so epoll_wait won't spuriously report the
                        // wakeup fd readable again on the next call.
                        self.event_fd.read();
                        awoken = true;
                        continue;
                    }
                    let sk_event = if (ev.events & linux_epoll::EPOLLPRI) != 0 {
                        SocketEvent::Exception
                    } else if (ev.events & linux_epoll::EPOLLOUT) != 0 {
                        SocketEvent::Write
                    } else {
                        SocketEvent::Read
                    };
                    kernel_sockets.push((fd as SocketDescriptor, sk_event));
                }
            }
            // Return here regardless of nfds: this epoll_wait call already fully honored
            // `sleep_duration` (including the "nothing happened, timed out" case), so falling
            // through to a second, separate Condvar wait below would silently double it.
            let now = Instant::now();
            let expired = self.collect_expired_timers(now);
            let mut combined = pending_sockets;
            combined.extend(kernel_sockets);
            return (awoken, expired, combined);
        }

        // No usable epoll instance (creation failed, or a non-Linux Unix target compiling this
        // module): fall back to a Condvar-based wait. wake_up()'s cond.notify_all() only matters
        // here — when the epoll branch above runs, nobody ever waits on `self.cond`.
        let (_new_guard, _timeout_res) = self.cond.wait_timeout(guard, sleep_duration).unwrap();
        let now = Instant::now();
        let was_awoken = self.event_fd.read() > 0;
        let expired = self.collect_expired_timers(now);
        let pending_sockets = std::mem::take(&mut *self.pending_socket_events.lock().unwrap());
        (was_awoken, expired, pending_sockets)
    }

    fn next_timer_delay(&self, now: Instant) -> Option<Duration> {
        let tfds = self.timer_fds.lock().unwrap();
        let mut min_delay: Option<Duration> = None;
        for tfd in tfds.values() {
            if let Some(target) = *tfd.next_fire.lock().unwrap() {
                let delay = if target > now {
                    target - now
                } else {
                    Duration::from_millis(0)
                };
                min_delay = Some(min_delay.map_or(delay, |m| m.min(delay)));
            }
        }
        min_delay
    }

    fn collect_expired_timers(&self, now: Instant) -> Vec<TimerId> {
        let tfds = self.timer_fds.lock().unwrap();
        let mut expired = Vec::new();
        for (id, tfd) in tfds.iter() {
            if tfd.check_and_fire(now) {
                expired.push(*id);
            }
        }
        expired
    }
}
impl Drop for EpollReactor {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        if self.epoll_fd >= 0 {
            unsafe {
                linux_epoll::close(self.epoll_fd);
            }
        }
    }
}

/// Linux/Unix cross-thread wakeup handle
#[derive(Clone)]
pub struct UnixEventDispatcherHandle {
    reactor: Arc<EpollReactor>,
}

impl EventDispatcherHandle for UnixEventDispatcherHandle {
    fn wake_up(&self) {
        self.reactor.wake_up();
    }
}

pub struct UnixEventDispatcher {
    reactor: Arc<EpollReactor>,
    pending_timers: Mutex<Vec<TimerId>>,
    native_filters: NativeEventFilterChain,
}

impl Default for UnixEventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl UnixEventDispatcher {
    pub fn new() -> Self {
        Self {
            reactor: Arc::new(EpollReactor::new()),
            pending_timers: Mutex::new(Vec::new()),
            native_filters: NativeEventFilterChain::new(),
        }
    }

    pub fn clone_handle(&self) -> UnixEventDispatcherHandle {
        UnixEventDispatcherHandle {
            reactor: Arc::clone(&self.reactor),
        }
    }
}

impl EventDispatcher for UnixEventDispatcher {
    // reactor.wake_up() is the single, unconditional path: every wakeup writes the real eventfd
    // (or, off Linux, the simulated counter + notifies the Condvar). There used to be a
    // `wakeup_pending: AtomicBool` dedup gate here that skipped the write when it thought a
    // wakeup was "already pending" — but it was only ever reset to false at the top of
    // process_events(), not at the moment a pending wakeup was actually drained. That let a
    // wake_up() called while delivering already-posted events (e.g. a queued slot calling
    // post_quit) observe a stale "still pending" flag left over from the *previous* iteration's
    // already-consumed wakeup, and silently skip writing the eventfd — a genuine lost wakeup,
    // distinct from (and found after fixing) the eventfd/epoll registration bug. The eventfd's
    // own accumulating counter already coalesces a burst of redundant wakeups on its own; this
    // flag added a second, racy bookkeeping layer on top for no correctness benefit.
    fn wake_up(&self) {
        self.reactor.wake_up();
    }

    fn clone_handle(&self) -> Arc<dyn EventDispatcherHandle> {
        Arc::new(self.clone_handle())
    }

    fn install_native_event_filter(&mut self, filter: Box<dyn NativeEventFilter>) {
        self.native_filters.install(filter);
    }

    fn filter_native_event(
        &mut self,
        event_type: &str,
        msg: &NativeMessage,
        result: &mut isize,
    ) -> bool {
        self.native_filters.filter_native(event_type, msg, result)
    }

    fn process_events(
        &mut self,
        can_wait: bool,
        next_timer_timeout: Option<Duration>,
    ) -> DispatchResult {
        let timeout = if can_wait {
            next_timer_timeout
        } else {
            Some(Duration::from_millis(0))
        };

        let (awoken, expired_timers, triggered_sockets) = self.reactor.epoll_wait(timeout);
        let had_socket_events = !triggered_sockets.is_empty();

        if had_socket_events {
            let notifiers = self.reactor.socket_notifiers.lock().unwrap();
            for (fd, ev) in triggered_sockets {
                if let Some(notifier) = notifiers.get(&(fd, ev)) {
                    notifier.notify();
                }
            }
        }

        if awoken {
            return DispatchResult::Awoken;
        }
        if !expired_timers.is_empty() {
            self.pending_timers.lock().unwrap().extend(expired_timers);
            return DispatchResult::Normal;
        }
        if had_socket_events {
            return DispatchResult::Normal;
        }
        // Nothing was awoken, no timer fired, and no socket activated: this poll found no
        // event, matching Win32EventDispatcher's contract (Timeout whenever !can_wait or the
        // wait itself timed out). Returning Normal here made every non-blocking
        // process_events(false) call look "handled" even when it did nothing, which
        // EventLoop::process_events surfaces as a spurious `true`.
        DispatchResult::Timeout
    }

    fn register_timer(&mut self, entry: &TimerEntry) {
        self.reactor.register_timer(
            entry.id,
            Duration::from_millis(entry.interval_ms),
            entry.single_shot,
        );
    }

    fn unregister_timer(&mut self, entry: &TimerEntry) {
        self.reactor.unregister_timer(entry.id);
    }

    fn register_socket_notifier(&mut self, notifier: &Arc<SocketNotifier>) {
        self.reactor.register_socket_notifier(notifier);
    }

    fn unregister_socket_notifier(&mut self, notifier: &Arc<SocketNotifier>) {
        self.reactor.unregister_socket_notifier(notifier);
    }

    fn send_timer_events(&mut self, registry: &mut TimerRegistry) {
        let pending = std::mem::take(&mut *self.pending_timers.lock().unwrap());
        for id in pending {
            let Some(entry) = registry.get(id) else {
                continue;
            };
            if entry.in_timer_event {
                continue;
            }
            let receiver = entry.receiver;
            let single_shot = entry.single_shot;

            if let Some(entry) = registry.get_mut(id) {
                entry.in_timer_event = true;
            }

            let handled = crate::object::with_object_mut(receiver, |obj| {
                let mut event = crate::event::Event::new(crate::event::EventKind::Timer {
                    timer_id: id.0 as u64,
                });
                obj.event(&mut event);
            })
            .is_some();
            if !handled {
                crate::timer::dispatch_single_shot_callback(receiver);
            }

            if single_shot {
                registry.unregister(id);
                self.reactor.unregister_timer(id);
            } else if let Some(entry) = registry.get_mut(id) {
                entry.in_timer_event = false;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unix_eventfd_write_read() {
        let efd = EventFd::new(0);
        assert_eq!(efd.read(), 0);

        efd.write(1);
        efd.write(2);
        assert_eq!(efd.read(), 3);
        // Fully drained: a second read observes nothing pending.
        assert_eq!(efd.read(), 0);
    }

    #[test]
    fn test_unix_dispatcher_cross_thread_wakeup() {
        let mut dispatcher = UnixEventDispatcher::new();
        let handle = dispatcher.clone_handle();

        let thread_handle = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            handle.wake_up();
        });

        let result = dispatcher.process_events(true, Some(Duration::from_millis(500)));
        assert_eq!(result, DispatchResult::Awoken);

        thread_handle.join().unwrap();
    }

    #[test]
    fn test_unix_dispatcher_timeout() {
        let mut dispatcher = UnixEventDispatcher::new();
        let start = Instant::now();
        let result = dispatcher.process_events(true, Some(Duration::from_millis(30)));
        assert_eq!(result, DispatchResult::Timeout);
        assert!(start.elapsed() >= Duration::from_millis(25));
    }

    #[test]
    fn test_unix_dispatcher_timer_integration() {
        let mut dispatcher = UnixEventDispatcher::new();
        let mut entry = TimerEntry::new(
            TimerId(101),
            20,
            0,
            crate::timer::TimerType::Precise,
            crate::object::ObjectId::next(),
        );
        entry.single_shot = true;
        dispatcher.register_timer(&entry);
        std::thread::sleep(Duration::from_millis(30));

        let result = dispatcher.process_events(false, None);
        assert_eq!(result, DispatchResult::Normal);

        dispatcher.unregister_timer(&entry);
    }

    #[test]
    fn test_unix_dispatcher_socket_notifier_activation() {
        let mut dispatcher = UnixEventDispatcher::new();
        let fd = 42; // Simulated socket FD (e.g. X11 or D-Bus)
        let notifier = Arc::new(SocketNotifier::new(fd, SocketEvent::Read));

        let triggered_fd = Arc::new(Mutex::new(None));
        let sink = Arc::clone(&triggered_fd);
        notifier.activated().connect(move |&sock| {
            *sink.lock().unwrap() = Some(sock);
        });

        dispatcher.register_socket_notifier(&notifier);

        // Simulate incoming socket data (EPOLLIN)
        dispatcher
            .reactor
            .trigger_socket_event(fd, SocketEvent::Read);

        // Dispatch events
        let res = dispatcher.process_events(false, None);
        assert_eq!(res, DispatchResult::Normal);
        assert_eq!(*triggered_fd.lock().unwrap(), Some(fd));

        // Disabling notifier should prevent signals
        *triggered_fd.lock().unwrap() = None;
        notifier.set_enabled(false);
        dispatcher
            .reactor
            .trigger_socket_event(fd, SocketEvent::Read);
        dispatcher.process_events(false, None);
        assert_eq!(*triggered_fd.lock().unwrap(), None);

        dispatcher.unregister_socket_notifier(&notifier);
    }
}
