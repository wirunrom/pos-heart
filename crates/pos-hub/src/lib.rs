//! The shop hub (multi-device phase 1): paired devices send
//! `pos_service::Request`s over the shop LAN with TLS; the hub runs them one
//! at a time on the single database owner, as the device, and pushes change
//! notices to every connected device. Plain threads and blocking I/O with
//! read timeouts, no async runtime (AGENTS.md).
//!
//! The application owns the database: it gives the hub a [`Submit`]
//! function that queues [`Job`]s to its worker, and the worker calls
//! [`Executor::run`] for each job. Pairing, token checks and requests all
//! go through that one queue.
pub mod discovery;
pub mod tls;
mod web;
pub mod webtls;
pub mod wire;

use pos_core::{DeviceContext, DeviceKind, Pos};
use pos_service::{Reply, Request, Topic, Versions};
use rustls::{ClientConnection, ServerConnection, StreamOwned};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    io::Write,
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;
use wire::{ClientMessage, Framer, Refusal, ServerMessage, WireError, write_frame};

/// How long a pairing code stays valid.
pub const PAIRING_TTL: Duration = Duration::from_secs(5 * 60);
/// Wrong codes allowed before the open pairing is cancelled.
const PAIRING_ATTEMPTS: u32 = 5;
const POLL: Duration = Duration::from_millis(100);

/// Work for the database owner.
pub enum Work {
    /// A paired device's request, run as that device on its register.
    Call {
        register: Uuid,
        request: Box<Request>,
    },
    /// Checks a device token (hashed); answers [`Done::Login`].
    Login { token_hash: String },
    /// Records a device after its pairing code was accepted.
    Pair {
        name: String,
        kind: DeviceKind,
        token_hash: String,
    },
}
pub enum Done {
    Reply(Box<Reply>),
    /// The device's register when the token is valid.
    Login(Option<Uuid>),
    Paired {
        register: Uuid,
    },
}
pub struct Job {
    pub device: Uuid,
    pub work: Work,
    pub reply: mpsc::Sender<Result<Done, WireError>>,
}
/// Queues a job to the database owner; false when it has stopped.
pub type Submit = Arc<dyn Fn(Job) -> bool + Send + Sync>;

pub fn token_hash(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Runs device jobs on the database owner's thread, keeping each device's
/// sign-in and register between its requests.
#[derive(Default)]
pub struct Executor {
    devices: HashMap<Uuid, DeviceContext>,
}
impl Executor {
    /// Runs one job; true when it changed shop data (a successful write or
    /// a pairing), so the counter's own screens can refresh.
    pub fn run(&mut self, pos: &mut Pos, job: Job, now: i64) -> bool {
        let writes = match &job.work {
            Work::Call { request, .. } => !pos_service::changes(request).is_empty(),
            Work::Pair { .. } => true,
            Work::Login { .. } => false,
        };
        let result = match job.work {
            Work::Call { register, request } => {
                let device = self
                    .devices
                    .entry(job.device)
                    .or_insert_with(|| DeviceContext::new(register).device(job.device));
                if device.register() != register {
                    *device = DeviceContext::new(register).device(job.device);
                }
                pos.as_device(device, |pos| pos_service::handle(pos, *request, now))
                    .map(|r| Done::Reply(Box::new(r)))
            }
            Work::Login { token_hash } => pos
                .device_login(job.device, &token_hash, now)
                .map(Done::Login),
            Work::Pair {
                name,
                kind,
                token_hash,
            } => {
                // A re-paired device starts signed out.
                self.devices.remove(&job.device);
                pos.pair_device(job.device, &name, kind, &token_hash, now)
                    .map(|d| Done::Paired {
                        register: d.register_id,
                    })
            }
        };
        let changed = writes && result.is_ok();
        // The device may have disconnected; nothing else to do.
        let _ = job.reply.send(result.map_err(|e| WireError::from(&e)));
        changed
    }
    /// Forgets a revoked device's sign-in.
    pub fn forget(&mut self, device: Uuid) {
        self.devices.remove(&device);
    }
}

struct Pairing {
    code: String,
    until: Instant,
    misses: u32,
}
struct Shared {
    stop: AtomicBool,
    versions: Mutex<Versions>,
    /// Change-notice senders of connected devices (connection number, sender).
    peers: Mutex<HashMap<Uuid, (u64, mpsc::Sender<Versions>)>>,
    pairing: Mutex<Option<Pairing>>,
    next_conn: AtomicU64,
    /// Verified web (iPad) tokens: device → (token hash, register).
    web: Mutex<HashMap<Uuid, (String, Uuid)>>,
}
impl Shared {
    fn notify(&self, request: &Request) {
        let versions = {
            let Ok(mut v) = self.versions.lock() else {
                return;
            };
            if v.record(request).is_empty() {
                return;
            }
            v.clone()
        };
        self.broadcast(versions);
    }
    fn touch(&self, topic: Topic) {
        let versions = {
            let Ok(mut v) = self.versions.lock() else {
                return;
            };
            v.bump(topic);
            v.clone()
        };
        self.broadcast(versions);
    }
    fn broadcast(&self, versions: Versions) {
        if let Ok(peers) = self.peers.lock() {
            for (_, tx) in peers.values() {
                let _ = tx.send(versions.clone());
            }
        }
    }
    /// Checks and consumes the open pairing code.
    fn take_code(&self, code: &str) -> bool {
        let Ok(mut pairing) = self.pairing.lock() else {
            return false;
        };
        let Some(open) = pairing.as_mut() else {
            return false;
        };
        if Instant::now() > open.until {
            *pairing = None;
            return false;
        }
        if open.code == code {
            *pairing = None;
            return true;
        }
        open.misses += 1;
        if open.misses >= PAIRING_ATTEMPTS {
            *pairing = None;
        }
        false
    }
}

fn stopped() -> WireError {
    WireError::new("Stopped", "The main register stopped")
}
fn ask(submit: &Submit, device: Uuid, work: Work) -> Result<Done, WireError> {
    let (tx, rx) = mpsc::channel();
    if !submit(Job {
        device,
        work,
        reply: tx,
    }) {
        return Err(stopped());
    }
    rx.recv().unwrap_or_else(|_| Err(stopped()))
}

/// Sends change notices and disconnects devices from the database owner's
/// thread (cheap to clone).
#[derive(Clone)]
pub struct Notifier(Arc<Shared>);
impl Notifier {
    /// Tells connected devices that `request` (run on the counter itself)
    /// changed data.
    pub fn notify(&self, request: &Request) {
        self.0.notify(request);
    }
    /// Tells connected devices that `topic` changed outside the protocol
    /// (the main register published its shop profile).
    pub fn touch(&self, topic: Topic) {
        self.0.touch(topic);
    }
    /// Closes a device's connection (after revoking it).
    pub fn disconnect(&self, device: Uuid) {
        if let Ok(mut peers) = self.0.peers.lock() {
            peers.remove(&device);
        }
        if let Ok(mut web) = self.0.web.lock() {
            web.remove(&device);
        }
    }
}
/// A running hub. Dropping it stops accepting and closes connections.
pub struct Hub {
    shared: Arc<Shared>,
    address: SocketAddr,
    fingerprint: [u8; 32],
    accept: Option<thread::JoinHandle<()>>,
    submit: Submit,
    web: Vec<thread::JoinHandle<()>>,
}
impl Hub {
    /// Starts accepting devices on `listener` with the hub `identity`.
    pub fn start(
        listener: TcpListener,
        identity: &tls::Identity,
        submit: Submit,
    ) -> std::io::Result<Self> {
        let address = listener.local_addr()?;
        let config = identity.server_config()?;
        listener.set_nonblocking(true)?;
        let shared = Arc::new(Shared {
            stop: AtomicBool::new(false),
            versions: Mutex::new(Versions::default()),
            peers: Mutex::new(HashMap::new()),
            pairing: Mutex::new(None),
            next_conn: AtomicU64::new(1),
            web: Mutex::new(HashMap::new()),
        });
        let hub_submit = submit.clone();
        let accept_shared = shared.clone();
        let accept = thread::Builder::new()
            .name("pos-hub-accept".into())
            .spawn(move || {
                while !accept_shared.stop.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let shared = accept_shared.clone();
                            let submit = submit.clone();
                            let config = config.clone();
                            let _ = thread::Builder::new()
                                .name("pos-hub-device".into())
                                .spawn(move || serve(stream, config, &shared, &submit));
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(50));
                        }
                        Err(_) => thread::sleep(Duration::from_millis(200)),
                    }
                }
            })?;
        Ok(Self {
            shared,
            address,
            fingerprint: identity.fingerprint(),
            accept: Some(accept),
            submit: hub_submit,
            web: Vec::new(),
        })
    }
    pub fn address(&self) -> SocketAddr {
        self.address
    }
    pub fn notifier(&self) -> Notifier {
        Notifier(self.shared.clone())
    }
    pub fn fingerprint(&self) -> [u8; 32] {
        self.fingerprint
    }
    /// Opens pairing for [`PAIRING_TTL`]; returns the 6-digit code to show.
    /// A new code replaces the previous one.
    pub fn open_pairing(&self) -> String {
        let n = Uuid::new_v4().as_u128() % 1_000_000;
        let code = format!("{n:06}");
        if let Ok(mut p) = self.shared.pairing.lock() {
            *p = Some(Pairing {
                code: code.clone(),
                until: Instant::now() + PAIRING_TTL,
                misses: 0,
            });
        }
        code
    }
    pub fn close_pairing(&self) {
        if let Ok(mut p) = self.shared.pairing.lock() {
            *p = None;
        }
    }
    /// Tells connected devices that `request` (run on the counter itself)
    /// changed data.
    pub fn notify(&self, request: &Request) {
        self.shared.notify(request);
    }
    /// Disconnects a device (after revoking it).
    pub fn disconnect(&self, device: Uuid) {
        self.notifier().disconnect(device);
    }
    /// Serves the iPad web app: `https` for the app and its API, `http` for
    /// the setup page and the certificate profile.
    pub fn start_web(
        &mut self,
        https: TcpListener,
        http: TcpListener,
        identity: &webtls::WebIdentity,
        ips: &[std::net::IpAddr],
        shop: &str,
    ) -> std::io::Result<()> {
        let https_port = https.local_addr()?.port();
        let config = identity.server_config(ips)?;
        let site = Arc::new(web::Site {
            shared: self.shared.clone(),
            submit: self.submit.clone(),
            profile: identity.mobileconfig(shop),
            ca: identity.ca_der().to_vec(),
            shop: shop.to_owned(),
            https_port,
        });
        self.web
            .push(web::listen(https, Some(config), site.clone())?);
        self.web.push(web::listen(http, None, site)?);
        Ok(())
    }
    pub fn connected(&self) -> Vec<Uuid> {
        self.shared
            .peers
            .lock()
            .map(|p| p.keys().copied().collect())
            .unwrap_or_default()
    }
}
impl Drop for Hub {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        if let Some(accept) = self.accept.take() {
            let _ = accept.join();
        }
        for web in self.web.drain(..) {
            let _ = web.join();
        }
    }
}

type ServerStream = StreamOwned<ServerConnection, TcpStream>;
fn send(stream: &mut ServerStream, message: &ServerMessage) -> bool {
    write_frame(stream, message).is_ok()
}
fn refuse(stream: &mut ServerStream, reason: Refusal) {
    send(stream, &ServerMessage::Refused(reason));
    let _ = stream.flush();
    let _ = stream.sock.shutdown(Shutdown::Both);
}
fn serve(tcp: TcpStream, config: Arc<rustls::ServerConfig>, shared: &Shared, submit: &Submit) {
    // Accepted sockets inherit the listener's non-blocking mode on BSDs.
    let mut tcp = tcp;
    if tcp.set_nonblocking(false).is_err() || tcp.set_nodelay(true).is_err() {
        return;
    }
    let Ok(mut conn) = ServerConnection::new(config) else {
        return;
    };
    if handshake(&mut conn, &mut tcp).is_err() || tcp.set_read_timeout(Some(POLL)).is_err() {
        return;
    }
    let mut stream = StreamOwned::new(conn, tcp);
    let mut framer = Framer::default();
    let started = Instant::now();
    // The first message decides what this connection is.
    let first = loop {
        match framer.poll::<ClientMessage>(&mut stream) {
            Ok(Some(m)) => break m,
            Ok(None) if started.elapsed() < Duration::from_secs(10) => {}
            _ => return,
        }
    };
    let (device, register) = match first {
        ClientMessage::Hello { protocol, .. } | ClientMessage::Pair { protocol, .. }
            if protocol != pos_service::PROTOCOL_VERSION =>
        {
            refuse(
                &mut stream,
                Refusal::Version {
                    hub: pos_service::PROTOCOL_VERSION,
                },
            );
            return;
        }
        ClientMessage::Pair {
            code,
            device,
            name,
            kind,
            ..
        } => {
            if !shared.take_code(&code) {
                refuse(&mut stream, Refusal::WrongCode);
                return;
            }
            let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
            let work = Work::Pair {
                name,
                kind,
                token_hash: token_hash(&token),
            };
            let message = match ask(submit, device, work) {
                Ok(Done::Paired { register }) => ServerMessage::Paired { token, register },
                Ok(_) => ServerMessage::Refused(Refusal::BadMessage),
                Err(e) => ServerMessage::Answer {
                    id: Uuid::nil(),
                    result: Err(e),
                },
            };
            send(&mut stream, &message);
            let _ = stream.flush();
            return;
        }
        ClientMessage::Hello { device, token, .. } => {
            let work = Work::Login {
                token_hash: token_hash(&token),
            };
            match ask(submit, device, work) {
                Ok(Done::Login(Some(register))) => (device, register),
                _ => {
                    refuse(&mut stream, Refusal::NotPaired);
                    return;
                }
            }
        }
        ClientMessage::Call { .. } => {
            refuse(&mut stream, Refusal::BadMessage);
            return;
        }
    };
    let versions = shared
        .versions
        .lock()
        .map(|v| v.clone())
        .unwrap_or_default();
    if !send(&mut stream, &ServerMessage::Welcome { register, versions }) {
        return;
    }
    // A newer connection of the same device replaces the older one.
    let (tx, rx) = mpsc::channel();
    let conn_id = shared.next_conn.fetch_add(1, Ordering::SeqCst);
    if let Ok(mut peers) = shared.peers.lock() {
        peers.insert(device, (conn_id, tx));
    }
    'serve: loop {
        if shared.stop.load(Ordering::SeqCst) {
            break;
        }
        // Change notices from other devices and the counter.
        loop {
            match rx.try_recv() {
                Ok(v) => {
                    if !send(&mut stream, &ServerMessage::Changed(v)) {
                        break 'serve;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                // Replaced by a newer connection, or disconnected (revoked).
                Err(mpsc::TryRecvError::Disconnected) => {
                    let _ = stream.sock.shutdown(Shutdown::Both);
                    break 'serve;
                }
            }
        }
        match framer.poll::<ClientMessage>(&mut stream) {
            Ok(None) => {}
            Ok(Some(ClientMessage::Call { id, request })) => {
                let work = Work::Call {
                    register,
                    request: request.clone(),
                };
                let result = match ask(submit, device, work) {
                    Ok(Done::Reply(reply)) => Ok(reply),
                    Ok(_) => Err(stopped()),
                    Err(e) => Err(e),
                };
                let ok = result.is_ok();
                if !send(&mut stream, &ServerMessage::Answer { id, result }) {
                    break;
                }
                if ok {
                    shared.notify(&request);
                }
            }
            Ok(Some(_)) | Err(_) => break,
        }
    }
    if let Ok(mut peers) = shared.peers.lock()
        && peers.get(&device).is_some_and(|(id, _)| *id == conn_id)
    {
        peers.remove(&device);
    }
}

type ClientStream = StreamOwned<ClientConnection, TcpStream>;
/// A device's TLS connection to the hub (blocking calls; change notices
/// that arrive between answers are collected for [`Client::changes`]).
pub struct Client {
    stream: ClientStream,
    framer: Framer,
    pub register: Uuid,
    pending: Vec<Versions>,
}
#[derive(Debug)]
pub enum ClientError {
    Frame(wire::FrameError),
    Refused(Refusal),
    Business(WireError),
    Protocol,
    /// No answer within the time limit.
    Timeout,
}
impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Frame(e) => write!(f, "{e}"),
            Self::Refused(r) => write!(f, "refused: {r:?}"),
            Self::Business(e) => write!(f, "{}", e.message),
            Self::Protocol => write!(f, "unexpected message from the hub"),
            Self::Timeout => write!(f, "the main register did not answer"),
        }
    }
}
impl std::error::Error for ClientError {}
impl From<wire::FrameError> for ClientError {
    fn from(e: wire::FrameError) -> Self {
        Self::Frame(e)
    }
}
impl From<std::io::Error> for ClientError {
    fn from(e: std::io::Error) -> Self {
        Self::Frame(wire::FrameError::Io(e))
    }
}
/// What a device keeps after pairing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paired {
    pub token: String,
    pub register: Uuid,
    /// The hub certificate to pin from now on.
    pub fingerprint: [u8; 32],
}
fn open(address: SocketAddr, pin: Option<[u8; 32]>) -> Result<ClientStream, ClientError> {
    let mut tcp = TcpStream::connect_timeout(&address, Duration::from_secs(5))?;
    tcp.set_nodelay(true)?;
    let mut conn = ClientConnection::new(tls::client_config(pin)?, tls::server_name())
        .map_err(std::io::Error::other)?;
    handshake(&mut conn, &mut tcp)?;
    tcp.set_read_timeout(Some(POLL))?;
    Ok(StreamOwned::new(conn, tcp))
}
/// Completes the TLS handshake with a generous blocking timeout, before the
/// short polling timeout applies (a timed-out read mid-handshake fails).
fn handshake<C, D>(conn: &mut C, tcp: &mut TcpStream) -> std::io::Result<()>
where
    C: std::ops::DerefMut<Target = rustls::ConnectionCommon<D>>,
    D: rustls::SideData,
{
    tcp.set_read_timeout(Some(Duration::from_secs(10)))?;
    while conn.is_handshaking() {
        conn.complete_io(tcp)?;
    }
    Ok(())
}
fn wait(
    stream: &mut ClientStream,
    framer: &mut Framer,
    limit: Duration,
) -> Result<ServerMessage, ClientError> {
    let started = Instant::now();
    loop {
        if let Some(m) = framer.poll::<ServerMessage>(stream)? {
            return Ok(m);
        }
        if started.elapsed() > limit {
            return Err(ClientError::Timeout);
        }
    }
}
impl Client {
    /// Pairs with the hub using the code it shows. The hub certificate is
    /// accepted on trust here and returned for pinning; compare
    /// [`tls::short`] of it with the hub's screen.
    pub fn pair(
        address: SocketAddr,
        code: &str,
        device: Uuid,
        name: &str,
        kind: DeviceKind,
    ) -> Result<Paired, ClientError> {
        let mut stream = open(address, None)?;
        let mut framer = Framer::default();
        write_frame(
            &mut stream,
            &ClientMessage::Pair {
                protocol: pos_service::PROTOCOL_VERSION,
                code: code.to_owned(),
                device,
                name: name.to_owned(),
                kind,
            },
        )?;
        let reply = wait(&mut stream, &mut framer, Duration::from_secs(15))?;
        let cert = stream
            .conn
            .peer_certificates()
            .and_then(|c| c.first())
            .map(|c| tls::fingerprint(c))
            .ok_or(ClientError::Protocol)?;
        match reply {
            ServerMessage::Paired { token, register } => Ok(Paired {
                token,
                register,
                fingerprint: cert,
            }),
            ServerMessage::Refused(r) => Err(ClientError::Refused(r)),
            ServerMessage::Answer { result: Err(e), .. } => Err(ClientError::Business(e)),
            _ => Err(ClientError::Protocol),
        }
    }
    /// Connects a paired device; only the pinned hub certificate is
    /// accepted.
    pub fn connect(
        address: SocketAddr,
        device: Uuid,
        token: &str,
        fingerprint: [u8; 32],
    ) -> Result<(Self, Versions), ClientError> {
        let mut stream = open(address, Some(fingerprint))?;
        let mut framer = Framer::default();
        write_frame(
            &mut stream,
            &ClientMessage::Hello {
                protocol: pos_service::PROTOCOL_VERSION,
                device,
                token: token.to_owned(),
            },
        )?;
        match wait(&mut stream, &mut framer, Duration::from_secs(15))? {
            ServerMessage::Welcome { register, versions } => Ok((
                Self {
                    stream,
                    framer,
                    register,
                    pending: Vec::new(),
                },
                versions,
            )),
            ServerMessage::Refused(r) => Err(ClientError::Refused(r)),
            _ => Err(ClientError::Protocol),
        }
    }
    /// Sends a request and waits for its answer. Resending with the same
    /// `id` after a dropped connection is safe for writes with a retry
    /// identity (payments, refunds, shifts).
    pub fn call(&mut self, id: Uuid, request: Request) -> Result<Reply, ClientError> {
        write_frame(
            &mut self.stream,
            &ClientMessage::Call {
                id,
                request: Box::new(request),
            },
        )?;
        loop {
            match wait(&mut self.stream, &mut self.framer, Duration::from_secs(30))? {
                ServerMessage::Answer { id: got, result } if got == id => {
                    return result.map(|r| *r).map_err(ClientError::Business);
                }
                ServerMessage::Changed(v) => self.pending.push(v),
                ServerMessage::Refused(r) => return Err(ClientError::Refused(r)),
                _ => return Err(ClientError::Protocol),
            }
        }
    }
    /// Change notices received so far (newest last), including any waiting
    /// on the connection now.
    pub fn changes(&mut self) -> Result<Vec<Versions>, ClientError> {
        while let Some(m) = self.framer.poll::<ServerMessage>(&mut self.stream)? {
            match m {
                ServerMessage::Changed(v) => self.pending.push(v),
                ServerMessage::Refused(r) => return Err(ClientError::Refused(r)),
                _ => return Err(ClientError::Protocol),
            }
        }
        Ok(std::mem::take(&mut self.pending))
    }
}
