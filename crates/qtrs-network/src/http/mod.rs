//! High-level HTTP/1.1 client subsystem matching Qt's `QNetworkAccessManager`,
//! `QNetworkRequest`, and `QNetworkReply`.

use std::collections::HashMap;
use std::fmt;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use qtrs_core::signal::Signal;
use qtrs_core::thread::future::{Future, Promise};
use qtrs_core::types::ByteArray;
use qtrs_core::variant::Variant;

// =============================================================================
// NetworkRequest Enums & Struct
// =============================================================================

/// Known standard HTTP headers (`QNetworkRequest::KnownHeaders`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KnownHeaders {
    ContentTypeHeader,
    ContentLengthHeader,
    LocationHeader,
    LastModifiedHeader,
    CookieHeader,
    SetCookieHeader,
    ContentDispositionHeader,
    UserAgentHeader,
    ServerHeader,
    IfModifiedSinceHeader,
    ETagHeader,
    IfMatchHeader,
    IfNoneMatchHeader,
    AuthorizationHeader,
}

impl KnownHeaders {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ContentTypeHeader => "Content-Type",
            Self::ContentLengthHeader => "Content-Length",
            Self::LocationHeader => "Location",
            Self::LastModifiedHeader => "Last-Modified",
            Self::CookieHeader => "Cookie",
            Self::SetCookieHeader => "Set-Cookie",
            Self::ContentDispositionHeader => "Content-Disposition",
            Self::UserAgentHeader => "User-Agent",
            Self::ServerHeader => "Server",
            Self::IfModifiedSinceHeader => "If-Modified-Since",
            Self::ETagHeader => "ETag",
            Self::IfMatchHeader => "If-Match",
            Self::IfNoneMatchHeader => "If-None-Match",
            Self::AuthorizationHeader => "Authorization",
        }
    }
}

/// Request attributes (`QNetworkRequest::Attribute`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequestAttribute {
    HttpStatusCodeAttribute,
    HttpReasonPhraseAttribute,
    RedirectionTargetAttribute,
    ConnectionEncryptedAttribute,
    SourceIsFromCacheAttribute,
    User(u32),
}

/// HTTP Request configuration matching `QNetworkRequest`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NetworkRequest {
    url: String,
    raw_headers: HashMap<String, ByteArray>,
    attributes: HashMap<RequestAttribute, Variant>,
    transfer_timeout: Duration,
}

impl NetworkRequest {
    pub fn new() -> Self {
        Self {
            url: String::new(),
            raw_headers: HashMap::new(),
            attributes: HashMap::new(),
            transfer_timeout: Duration::from_secs(30),
        }
    }

    pub fn from_url(url: impl Into<String>) -> Self {
        let mut req = Self::new();
        req.url = url.into();
        req
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn set_url(&mut self, url: impl Into<String>) {
        self.url = url.into();
    }

    pub fn header(&self, header: KnownHeaders) -> Option<String> {
        self.raw_headers
            .get(&header.as_str().to_lowercase())
            .map(|b| b.to_string_lossy().into_owned())
    }

    pub fn set_header(&mut self, header: KnownHeaders, value: impl AsRef<str>) {
        self.raw_headers.insert(
            header.as_str().to_lowercase(),
            ByteArray::from(value.as_ref().as_bytes()),
        );
    }

    pub fn raw_header(&self, name: &str) -> Option<&ByteArray> {
        self.raw_headers.get(&name.to_lowercase())
    }

    pub fn set_raw_header(&mut self, name: &str, value: ByteArray) {
        self.raw_headers.insert(name.to_lowercase(), value);
    }

    pub fn raw_header_list(&self) -> Vec<ByteArray> {
        self.raw_headers
            .keys()
            .map(|k| ByteArray::from(k.as_bytes()))
            .collect()
    }

    pub fn attribute(&self, code: RequestAttribute) -> Option<&Variant> {
        self.attributes.get(&code)
    }

    pub fn set_attribute(&mut self, code: RequestAttribute, value: Variant) {
        self.attributes.insert(code, value);
    }

    pub fn transfer_timeout(&self) -> Duration {
        self.transfer_timeout
    }

    pub fn set_transfer_timeout(&mut self, timeout: Duration) {
        self.transfer_timeout = timeout;
    }
}

// =============================================================================
// NetworkReply Enums & Struct
// =============================================================================

/// Network response error codes (`QNetworkReply::NetworkError`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NetworkError {
    NoError = 0,
    ConnectionRefusedError = 1,
    RemoteHostClosedError = 2,
    HostNotFoundError = 3,
    TimeoutError = 4,
    OperationCanceledError = 5,
    SslHandshakeFailedError = 6,
    ContentAccessDenied = 201,
    ContentNotFoundError = 203,
    InternalServerError = 401,
    ServiceUnavailableError = 403,
    UnknownNetworkError = 99,
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for NetworkError {}

/// Shared internal state of `NetworkReply`.
struct ReplyInner {
    status_code: u16,
    reason_phrase: String,
    raw_headers: Vec<(ByteArray, ByteArray)>,
    body: Vec<u8>,
    error: NetworkError,
    error_string: String,
    is_finished: bool,
    is_running: bool,
    is_aborted: bool,
}

/// Asynchronous network reply matching `QNetworkReply`.
pub struct NetworkReply {
    request: NetworkRequest,
    inner: Arc<Mutex<ReplyInner>>,

    // Qt Signals
    pub finished: Signal<()>,
    pub ready_read: Signal<()>,
    pub download_progress: Signal<(i64, i64)>,
    pub upload_progress: Signal<(i64, i64)>,
    pub error_occurred: Signal<NetworkError>,
}

impl NetworkReply {
    fn new(request: NetworkRequest) -> Self {
        Self {
            request,
            inner: Arc::new(Mutex::new(ReplyInner {
                status_code: 0,
                reason_phrase: String::new(),
                raw_headers: Vec::new(),
                body: Vec::new(),
                error: NetworkError::NoError,
                error_string: String::new(),
                is_finished: false,
                is_running: true,
                is_aborted: false,
            })),
            finished: Signal::new(),
            ready_read: Signal::new(),
            download_progress: Signal::new(),
            upload_progress: Signal::new(),
            error_occurred: Signal::new(),
        }
    }

    #[inline]
    pub fn request(&self) -> &NetworkRequest {
        &self.request
    }

    #[inline]
    pub fn status_code(&self) -> u16 {
        self.inner.lock().unwrap().status_code
    }

    #[inline]
    pub fn reason_phrase(&self) -> String {
        self.inner.lock().unwrap().reason_phrase.clone()
    }

    #[inline]
    pub fn is_finished(&self) -> bool {
        self.inner.lock().unwrap().is_finished
    }

    #[inline]
    pub fn is_running(&self) -> bool {
        self.inner.lock().unwrap().is_running
    }

    #[inline]
    pub fn error(&self) -> NetworkError {
        self.inner.lock().unwrap().error
    }

    #[inline]
    pub fn error_string(&self) -> String {
        self.inner.lock().unwrap().error_string.clone()
    }

    #[inline]
    pub fn read_all(&self) -> ByteArray {
        let lock = self.inner.lock().unwrap();
        ByteArray::from(lock.body.clone())
    }

    pub fn raw_header(&self, name: &str) -> Option<ByteArray> {
        let name_lower = name.to_lowercase();
        let lock = self.inner.lock().unwrap();
        for (k, v) in &lock.raw_headers {
            if k.to_string_lossy().to_lowercase() == name_lower {
                return Some(v.clone());
            }
        }
        None
    }

    pub fn header(&self, header: KnownHeaders) -> Option<String> {
        self.raw_header(header.as_str()).map(|b| b.to_string_lossy().into_owned())
    }

    pub fn raw_header_pairs(&self) -> Vec<(ByteArray, ByteArray)> {
        self.inner.lock().unwrap().raw_headers.clone()
    }

    pub fn abort(&self) {
        let mut lock = self.inner.lock().unwrap();
        if !lock.is_finished {
            lock.is_aborted = true;
            lock.is_running = false;
            lock.is_finished = true;
            lock.error = NetworkError::OperationCanceledError;
            lock.error_string = "Operation canceled".to_string();
            drop(lock);
            self.error_occurred.emit(&NetworkError::OperationCanceledError);
            self.finished.emit(&());
        }
    }
}

// =============================================================================
// NetworkAccessManager (QNetworkAccessManager)
// =============================================================================

/// HTTP operation verb (`QNetworkAccessManager::Operation`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpOperation {
    HeadOperation,
    GetOperation,
    PutOperation,
    PostOperation,
    DeleteOperation,
    CustomOperation,
}

/// Central network client engine matching `QNetworkAccessManager`.
#[derive(Clone)]
pub struct NetworkAccessManager {
    // Qt Signals
    pub finished: Signal<Arc<NetworkReply>>,
}

impl Default for NetworkAccessManager {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkAccessManager {
    pub fn new() -> Self {
        Self {
            finished: Signal::new(),
        }
    }

    /// Posts an HTTP GET request (`get`).
    pub fn get(&self, request: &NetworkRequest) -> Arc<NetworkReply> {
        self.execute_request(HttpOperation::GetOperation, request, None)
    }

    /// Posts an HTTP POST request (`post`).
    pub fn post(&self, request: &NetworkRequest, data: &[u8]) -> Arc<NetworkReply> {
        self.execute_request(HttpOperation::PostOperation, request, Some(data.to_vec()))
    }

    /// Posts an HTTP PUT request (`put`).
    pub fn put(&self, request: &NetworkRequest, data: &[u8]) -> Arc<NetworkReply> {
        self.execute_request(HttpOperation::PutOperation, request, Some(data.to_vec()))
    }

    /// Posts an HTTP DELETE request (`deleteResource`).
    pub fn delete_resource(&self, request: &NetworkRequest) -> Arc<NetworkReply> {
        self.execute_request(HttpOperation::DeleteOperation, request, None)
    }

    /// Posts an HTTP HEAD request (`head`).
    pub fn head(&self, request: &NetworkRequest) -> Arc<NetworkReply> {
        self.execute_request(HttpOperation::HeadOperation, request, None)
    }

    /// Sends a custom verb HTTP request (`sendCustomRequest`).
    pub fn send_custom_request(
        &self,
        request: &NetworkRequest,
        verb: &str,
        data: Option<&[u8]>,
    ) -> Arc<NetworkReply> {
        self.execute_request_custom(verb, request, data.map(|d| d.to_vec()))
    }

    /// Asynchronous GET returning a `qtrs_core::thread::future::Future`.
    pub fn get_async(&self, request: &NetworkRequest) -> Future<Result<ByteArray, NetworkError>> {
        let reply = self.get(request);
        let reply_clone = Arc::clone(&reply);
        let promise = Promise::new();
        let fut = promise.future();

        reply.finished.connect(move |_| {
            if reply_clone.error() == NetworkError::NoError {
                promise.set_value(Ok(reply_clone.read_all()));
            } else {
                promise.set_value(Err(reply_clone.error()));
            }
        });

        fut
    }

    /// Asynchronous POST returning a `qtrs_core::thread::future::Future`.
    pub fn post_async(
        &self,
        request: &NetworkRequest,
        data: &[u8],
    ) -> Future<Result<ByteArray, NetworkError>> {
        let reply = self.post(request, data);
        let reply_clone = Arc::clone(&reply);
        let promise = Promise::new();
        let fut = promise.future();

        reply.finished.connect(move |_| {
            if reply_clone.error() == NetworkError::NoError {
                promise.set_value(Ok(reply_clone.read_all()));
            } else {
                promise.set_value(Err(reply_clone.error()));
            }
        });

        fut
    }

    fn execute_request(
        &self,
        op: HttpOperation,
        request: &NetworkRequest,
        body: Option<Vec<u8>>,
    ) -> Arc<NetworkReply> {
        let verb = match op {
            HttpOperation::GetOperation => "GET",
            HttpOperation::PostOperation => "POST",
            HttpOperation::PutOperation => "PUT",
            HttpOperation::DeleteOperation => "DELETE",
            HttpOperation::HeadOperation => "HEAD",
            HttpOperation::CustomOperation => "CUSTOM",
        };
        self.execute_request_custom(verb, request, body)
    }

    fn execute_request_custom(
        &self,
        verb: &str,
        request: &NetworkRequest,
        body: Option<Vec<u8>>,
    ) -> Arc<NetworkReply> {
        let reply = Arc::new(NetworkReply::new(request.clone()));
        let reply_clone = Arc::clone(&reply);
        let manager_sig = self.finished.clone();

        let req_copy = request.clone();
        let verb_copy = verb.to_string();

        thread::spawn(move || {
            perform_http_dispatch(verb_copy, req_copy, body, reply_clone.clone());
            manager_sig.emit(&reply_clone);
        });

        reply
    }
}

// =============================================================================
// HTTP Multipart Form Data (QHttpMultiPart & QHttpPart)
// =============================================================================

/// Single MIME part in an HTTP multipart entity (`QHttpPart`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HttpPart {
    headers: HashMap<String, ByteArray>,
    body: Vec<u8>,
}

impl HttpPart {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_header(&mut self, header: KnownHeaders, value: impl AsRef<str>) {
        self.headers.insert(
            header.as_str().to_lowercase(),
            ByteArray::from(value.as_ref().as_bytes()),
        );
    }

    pub fn set_raw_header(&mut self, name: &str, value: ByteArray) {
        self.headers.insert(name.to_lowercase(), value);
    }

    pub fn set_body(&mut self, body: &[u8]) {
        self.body = body.to_vec();
    }
}

/// MIME multipart container matching `QHttpMultiPart`.
#[derive(Debug, Clone)]
pub struct HttpMultiPart {
    boundary: String,
    parts: Vec<HttpPart>,
}

impl Default for HttpMultiPart {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpMultiPart {
    pub fn new() -> Self {
        Self {
            boundary: format!("----qtrsBoundary{}", std::time::SystemTime::now().elapsed().unwrap_or_default().as_millis()),
            parts: Vec::new(),
        }
    }

    pub fn boundary(&self) -> &str {
        &self.boundary
    }

    pub fn set_boundary(&mut self, boundary: impl Into<String>) {
        self.boundary = boundary.into();
    }

    pub fn append(&mut self, part: HttpPart) {
        self.parts.push(part);
    }

    /// Formats the complete multipart payload.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        for part in &self.parts {
            buf.extend_from_slice(b"--");
            buf.extend_from_slice(self.boundary.as_bytes());
            buf.extend_from_slice(b"\r\n");

            for (k, v) in &part.headers {
                buf.extend_from_slice(k.as_bytes());
                buf.extend_from_slice(b": ");
                buf.extend_from_slice(v.as_bytes());
                buf.extend_from_slice(b"\r\n");
            }
            buf.extend_from_slice(b"\r\n");
            buf.extend_from_slice(&part.body);
            buf.extend_from_slice(b"\r\n");
        }
        buf.extend_from_slice(b"--");
        buf.extend_from_slice(self.boundary.as_bytes());
        buf.extend_from_slice(b"--\r\n");
        buf
    }
}

// =============================================================================
// HTTP Dispatch Engine
// =============================================================================

fn perform_http_dispatch(
    verb: String,
    request: NetworkRequest,
    body: Option<Vec<u8>>,
    reply: Arc<NetworkReply>,
) {
    let url = request.url();
    let stripped = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);

    let (host_port, path) = match stripped.split_once('/') {
        Some((hp, p)) => (hp, format!("/{}", p)),
        None => (stripped, "/".to_string()),
    };

    let (host, port) = match host_port.split_once(':') {
        Some((h, p)) => match p.parse::<u16>() {
            Ok(port_num) => (h, port_num),
            Err(_) => {
                fail_reply(reply, NetworkError::HostNotFoundError, "Invalid port");
                return;
            }
        },
        None => {
            let default_port = if url.starts_with("https://") { 443 } else { 80 };
            (host_port, default_port)
        }
    };

    let target = format!("{}:{}", host, port);
    let socket_addr = match target.parse::<std::net::SocketAddr>() {
        Ok(sa) => sa,
        Err(_) => match target.to_socket_addrs() {
            Ok(mut iter) => match iter.next() {
                Some(sa) => sa,
                None => {
                    fail_reply(reply, NetworkError::HostNotFoundError, "Host not found");
                    return;
                }
            },
            Err(e) => {
                fail_reply(reply, NetworkError::HostNotFoundError, &e.to_string());
                return;
            }
        },
    };

    let mut stream = match TcpStream::connect_timeout(&socket_addr, request.transfer_timeout()) {
        Ok(s) => s,
        Err(e) => {
            fail_reply(reply, NetworkError::ConnectionRefusedError, &e.to_string());
            return;
        }
    };

    let _ = stream.set_nodelay(true);

    // Build HTTP request
    let mut req_str = format!("{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n", verb, path, host);

    // Attach headers
    for (k, v) in &request.raw_headers {
        req_str.push_str(&format!("{}: {}\r\n", k, v.to_string_lossy()));
    }

    if let Some(b) = &body {
        if !request.raw_headers.contains_key("content-length") {
            req_str.push_str(&format!("content-length: {}\r\n", b.len()));
        }
    }
    req_str.push_str("\r\n");

    if stream.write_all(req_str.as_bytes()).is_err() {
        fail_reply(reply, NetworkError::RemoteHostClosedError, "Failed to send request headers");
        return;
    }

    if let Some(b) = &body {
        if stream.write_all(b).is_err() {
            fail_reply(reply, NetworkError::RemoteHostClosedError, "Failed to send request body");
            return;
        }
    }

    // Read HTTP response
    let mut response_bytes = Vec::new();
    let mut temp = [0u8; 8192];
    loop {
        match stream.read(&mut temp) {
            Ok(0) => break,
            Ok(n) => {
                response_bytes.extend_from_slice(&temp[..n]);
                reply.ready_read.emit(&());
            }
            Err(_) => break,
        }
    }

    // Parse response headers and body
    let mut split_pos = None;
    for i in 0..response_bytes.len().saturating_sub(3) {
        if &response_bytes[i..i + 4] == b"\r\n\r\n" {
            split_pos = Some(i);
            break;
        }
    }

    let (header_bytes, body_bytes) = match split_pos {
        Some(pos) => (&response_bytes[..pos], &response_bytes[pos + 4..]),
        None => (response_bytes.as_slice(), [].as_slice()),
    };

    let header_str = String::from_utf8_lossy(header_bytes);
    let mut lines = header_str.lines();

    let mut status_code = 200u16;
    let mut reason_phrase = "OK".to_string();

    if let Some(status_line) = lines.next() {
        let parts: Vec<&str> = status_line.split_whitespace().collect();
        if parts.len() >= 2 {
            if let Ok(code) = parts[1].parse::<u16>() {
                status_code = code;
            }
        }
        if parts.len() >= 3 {
            reason_phrase = parts[2..].join(" ");
        }
    }

    let mut parsed_headers = Vec::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            parsed_headers.push((
                ByteArray::from(k.trim().as_bytes()),
                ByteArray::from(v.trim().as_bytes()),
            ));
        }
    }

    let error_code = match status_code {
        200..=299 => NetworkError::NoError,
        403 => NetworkError::ContentAccessDenied,
        404 => NetworkError::ContentNotFoundError,
        500 => NetworkError::InternalServerError,
        503 => NetworkError::ServiceUnavailableError,
        _ => NetworkError::NoError,
    };

    {
        let mut lock = reply.inner.lock().unwrap();
        lock.status_code = status_code;
        lock.reason_phrase = reason_phrase;
        lock.raw_headers = parsed_headers;
        lock.body = body_bytes.to_vec();
        lock.is_running = false;
        lock.is_finished = true;
        lock.error = error_code;
    }

    reply.finished.emit(&());
}

fn fail_reply(reply: Arc<NetworkReply>, err: NetworkError, msg: &str) {
    {
        let mut lock = reply.inner.lock().unwrap();
        lock.error = err;
        lock.error_string = msg.to_string();
        lock.is_running = false;
        lock.is_finished = true;
    }
    reply.error_occurred.emit(&err);
    reply.finished.emit(&());
}
