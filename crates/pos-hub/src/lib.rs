//! The shop hub (multi-device phase 1): paired devices send
//! `pos_service::Request`s over the shop LAN; the hub runs them one at a
//! time on the single database owner, as the device, and pushes change
//! notices to every connected device. Plain threads and blocking I/O, no
//! async runtime (AGENTS.md).
//!
//! The application owns the database: it gives the hub a [`Submit`]
//! function that queues [`Job`]s to its worker, and the worker calls
//! [`Executor::run`] for each job.
pub mod wire;

use pos_core::{DeviceContext, Pos};
use pos_service::{Reply, Request, Versions};
use std::{
    collections::HashMap,
    io::{BufReader, BufWriter},
    net::{Shutdown, TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
use uuid::Uuid;
use wire::{ClientMessage, Refusal, ServerMessage, WireError, read_frame, write_frame};

/// One device request waiting for the database owner.
pub struct Job {
    pub device: Uuid,
    pub register: Uuid,
    pub request: Request,
    pub reply: mpsc::Sender<Result<Reply, WireError>>,
}
/// Queues a job to the database owner; false when it has stopped.
pub type Submit = Arc<dyn Fn(Job) -> bool + Send + Sync>;
/// Checks a device's token; Some(register) when it is paired and active.
pub type Authorize = Arc<dyn Fn(Uuid, &str) -> Option<Uuid> + Send + Sync>;

/// Runs device jobs on the database owner's thread, keeping each device's
/// sign-in and register between its requests.
#[derive(Default)]
pub struct Executor {
    devices: HashMap<Uuid, DeviceContext>,
}
impl Executor {
    pub fn run(&mut self, pos: &mut Pos, job: Job, now: i64) {
        let device = self
            .devices
            .entry(job.device)
            .or_insert_with(|| DeviceContext::new(job.register));
        if device.register() != job.register {
            // Moved to another register on the hub: start a fresh session.
            *device = DeviceContext::new(job.register);
        }
        let result = pos
            .as_device(device, |pos| pos_service::handle(pos, job.request, now))
            .map_err(|e| WireError::from(&e));
        // The device may have disconnected; nothing else to do.
        let _ = job.reply.send(result);
    }
    /// Forgets a revoked device's sign-in.
    pub fn forget(&mut self, device: Uuid) {
        self.devices.remove(&device);
    }
}

struct Shared {
    stop: AtomicBool,
    versions: Mutex<Versions>,
    /// Writers of connected devices, for change notices.
    peers: Mutex<HashMap<Uuid, Arc<Mutex<BufWriter<TcpStream>>>>>,
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
        let peers: Vec<_> = match self.peers.lock() {
            Ok(p) => p.values().cloned().collect(),
            Err(_) => return,
        };
        for peer in peers {
            if let Ok(mut w) = peer.lock() {
                let _ = write_frame(&mut *w, &ServerMessage::Changed(versions.clone()));
            }
        }
    }
}

/// A running hub. Dropping it stops accepting and closes connections.
pub struct Hub {
    shared: Arc<Shared>,
    address: std::net::SocketAddr,
    accept: Option<thread::JoinHandle<()>>,
}
impl Hub {
    /// Starts accepting devices on `listener`.
    pub fn start(
        listener: TcpListener,
        authorize: Authorize,
        submit: Submit,
    ) -> std::io::Result<Self> {
        let address = listener.local_addr()?;
        listener.set_nonblocking(true)?;
        let shared = Arc::new(Shared {
            stop: AtomicBool::new(false),
            versions: Mutex::new(Versions::default()),
            peers: Mutex::new(HashMap::new()),
        });
        let accept_shared = shared.clone();
        let accept = thread::Builder::new()
            .name("pos-hub-accept".into())
            .spawn(move || {
                while !accept_shared.stop.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let shared = accept_shared.clone();
                            let authorize = authorize.clone();
                            let submit = submit.clone();
                            let _ = thread::Builder::new()
                                .name("pos-hub-device".into())
                                .spawn(move || serve(stream, &shared, &authorize, &submit));
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
            accept: Some(accept),
        })
    }
    pub fn address(&self) -> std::net::SocketAddr {
        self.address
    }
    /// Tells connected devices that `request` (run on the counter itself)
    /// changed data.
    pub fn notify(&self, request: &Request) {
        self.shared.notify(request);
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
        if let Ok(peers) = self.shared.peers.lock() {
            for peer in peers.values() {
                if let Ok(w) = peer.lock() {
                    let _ = w.get_ref().shutdown(Shutdown::Both);
                }
            }
        }
        if let Some(accept) = self.accept.take() {
            let _ = accept.join();
        }
    }
}

fn serve(stream: TcpStream, shared: &Shared, authorize: &Authorize, submit: &Submit) {
    // Accepted sockets inherit the listener's non-blocking mode on BSDs.
    if stream.set_nonblocking(false).is_err() {
        return;
    }
    let _ = stream.set_nodelay(true);
    let Ok(read_half) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(read_half);
    let writer = Arc::new(Mutex::new(BufWriter::new(stream)));
    let refuse = |reason: Refusal| {
        if let Ok(mut w) = writer.lock() {
            let _ = write_frame(&mut *w, &ServerMessage::Refused(reason));
            let _ = w.get_ref().shutdown(Shutdown::Both);
        }
    };
    let (device, register) = match read_frame::<ClientMessage>(&mut reader) {
        Ok(ClientMessage::Hello {
            protocol,
            device,
            token,
        }) => {
            if protocol != pos_service::PROTOCOL_VERSION {
                refuse(Refusal::Version {
                    hub: pos_service::PROTOCOL_VERSION,
                });
                return;
            }
            match authorize(device, &token) {
                Some(register) => (device, register),
                None => {
                    refuse(Refusal::NotPaired);
                    return;
                }
            }
        }
        _ => {
            refuse(Refusal::BadMessage);
            return;
        }
    };
    let versions = shared
        .versions
        .lock()
        .map(|v| v.clone())
        .unwrap_or_default();
    {
        let Ok(mut w) = writer.lock() else {
            return;
        };
        if write_frame(&mut *w, &ServerMessage::Welcome { register, versions }).is_err() {
            return;
        }
    }
    // A newer connection of the same device replaces the older one.
    if let Ok(mut peers) = shared.peers.lock()
        && let Some(old) = peers.insert(device, writer.clone())
        && let Ok(w) = old.lock()
    {
        let _ = w.get_ref().shutdown(Shutdown::Both);
    }
    while let Ok(ClientMessage::Call { id, request }) = read_frame::<ClientMessage>(&mut reader) {
        if shared.stop.load(Ordering::SeqCst) {
            break;
        }
        let (tx, rx) = mpsc::channel();
        let queued = submit(Job {
            device,
            register,
            request: (*request).clone(),
            reply: tx,
        });
        let result = if queued {
            rx.recv().unwrap_or_else(|_| {
                Err(WireError {
                    kind: "Stopped".into(),
                    message: "The main register stopped".into(),
                })
            })
        } else {
            Err(WireError {
                kind: "Stopped".into(),
                message: "The main register stopped".into(),
            })
        };
        let ok = result.is_ok();
        let sent = writer.lock().map(|mut w| {
            write_frame(
                &mut *w,
                &ServerMessage::Answer {
                    id,
                    result: result.map(Box::new),
                },
            )
        });
        if ok {
            shared.notify(&request);
        }
        if !matches!(sent, Ok(Ok(()))) {
            break;
        }
    }
    if let Ok(mut peers) = shared.peers.lock()
        && peers.get(&device).is_some_and(|w| Arc::ptr_eq(w, &writer))
    {
        peers.remove(&device);
    }
}

/// A device's connection to the hub (blocking calls; change notices that
/// arrive between answers are collected for [`Client::changes`]).
pub struct Client {
    reader: BufReader<TcpStream>,
    writer: BufWriter<TcpStream>,
    pub register: Uuid,
    pending: Vec<Versions>,
}
#[derive(Debug)]
pub enum ClientError {
    Frame(wire::FrameError),
    Refused(Refusal),
    Business(WireError),
    Protocol,
}
impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Frame(e) => write!(f, "{e}"),
            Self::Refused(r) => write!(f, "refused: {r:?}"),
            Self::Business(e) => write!(f, "{}", e.message),
            Self::Protocol => write!(f, "unexpected message from the hub"),
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
impl Client {
    pub fn connect(
        address: std::net::SocketAddr,
        device: Uuid,
        token: &str,
    ) -> Result<(Self, Versions), ClientError> {
        let stream = TcpStream::connect_timeout(&address, Duration::from_secs(5))?;
        stream.set_nodelay(true)?;
        let mut writer = BufWriter::new(stream.try_clone()?);
        let mut reader = BufReader::new(stream);
        write_frame(
            &mut writer,
            &ClientMessage::Hello {
                protocol: pos_service::PROTOCOL_VERSION,
                device,
                token: token.to_owned(),
            },
        )?;
        match read_frame::<ServerMessage>(&mut reader)? {
            ServerMessage::Welcome { register, versions } => Ok((
                Self {
                    reader,
                    writer,
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
            &mut self.writer,
            &ClientMessage::Call {
                id,
                request: Box::new(request),
            },
        )?;
        loop {
            match read_frame::<ServerMessage>(&mut self.reader)? {
                ServerMessage::Answer { id: got, result } if got == id => {
                    return result.map(|r| *r).map_err(ClientError::Business);
                }
                ServerMessage::Changed(v) => self.pending.push(v),
                ServerMessage::Refused(r) => return Err(ClientError::Refused(r)),
                _ => return Err(ClientError::Protocol),
            }
        }
    }
    /// Change notices received so far (newest last).
    pub fn changes(&mut self) -> Vec<Versions> {
        std::mem::take(&mut self.pending)
    }
}
