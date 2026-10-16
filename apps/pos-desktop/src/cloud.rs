//! Cloud sync on the main register (multi-device phase 2). Off until the
//! owner links the shop with a code from the cloud dashboard. A background
//! thread uploads the outbox in batches and applies the owner's menu edits;
//! the database work runs as jobs on the worker between the counter's own
//! commands, like paired devices. Selling never waits for the cloud: while
//! offline the outbox simply grows.
//!
//! `cloud.json` (server, hub ID, token, shop) is a device file beside the
//! database: never in backups, since the token is a credential.
use pos_core::{OutboxItem, Pos};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
use uuid::Uuid;

#[cfg(test)]
#[path = "cloud_tests.rs"]
mod cloud_tests;

pub const FILE: &str = "cloud.json";
/// Records per upload request (the server takes up to 1000).
const BATCH: u32 = 500;
/// Quiet period between syncs, and after a failure.
const IDLE: Duration = Duration::from_secs(30);
const RETRY: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudFile {
    pub server: String,
    pub hub: Uuid,
    pub token: String,
    pub shop: Uuid,
    pub shop_name: String,
    /// Backup key derived from the owner's backup password (hex), with
    /// its salt; the password itself is never stored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup_salt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_backup: Option<i64>,
}
impl CloudFile {
    pub fn key(&self) -> Option<crate::cloud_backup::Key> {
        crate::cloud_backup::Key::from_hex(
            self.backup_salt.as_deref()?,
            self.backup_key.as_deref()?,
        )
    }
    pub fn load(dir: &Path) -> Option<Self> {
        let bytes = std::fs::read(dir.join(FILE)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }
    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        let path = dir.join(FILE);
        std::fs::write(&path, serde_json::to_vec_pretty(self)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }
    pub fn remove(dir: &Path) -> std::io::Result<()> {
        match std::fs::remove_file(dir.join(FILE)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
    fn auth(&self) -> String {
        format!("Bearer {}.{}", self.hub, self.token)
    }
    fn url(&self, path: &str) -> String {
        format!("{}/api/v1{path}", self.server)
    }
}

/// An owner's edit waiting on the server.
#[derive(Clone, Debug, Deserialize)]
pub struct Change {
    pub seq: i64,
    pub kind: String,
    pub id: String,
    pub patch: Map<String, Value>,
}
/// Database work for the sync thread, run on the worker.
pub enum Work {
    Batch(u32),
    Ack(Vec<i64>),
    Apply(Vec<Change>),
    Requeue,
    Pending,
    /// A recovery bundle of the data folder (first) at the new folder (second).
    Snapshot(std::path::PathBuf, std::path::PathBuf),
}
pub enum Done {
    Batch(Vec<OutboxItem>),
    /// (seq, "applied" or why it was refused) per change.
    Applied(Vec<(i64, String)>),
    Pending(u64),
    Ok,
}
pub struct Job {
    pub work: Work,
    pub reply: mpsc::Sender<Result<Done, String>>,
}
pub type Submit = Arc<dyn Fn(Job) -> bool + Send + Sync>;

/// Runs one job on the worker; true when owner edits changed the catalog.
pub fn run(pos: &mut Pos, job: Job, now: i64) -> bool {
    let mut changed = false;
    let result = match job.work {
        Work::Batch(n) => pos.outbox_batch(n).map(Done::Batch),
        Work::Ack(seqs) => pos.outbox_ack(&seqs).map(|()| Done::Ok),
        Work::Requeue => pos.requeue_all().map(|()| Done::Ok),
        Work::Pending => pos.outbox_len().map(Done::Pending),
        // The owner opted in by setting a backup password: scheduled
        // maintenance, like automatic local backups.
        Work::Snapshot(data, dest) => {
            crate::backup::create_maintenance(pos, &data, &dest).map(|_| Done::Ok)
        }
        Work::Apply(changes) => Ok(Done::Applied(
            changes
                .into_iter()
                .map(|c| {
                    let outcome = match pos.apply_cloud_change(&c.kind, &c.id, &c.patch, now) {
                        Ok(()) => {
                            changed = true;
                            "applied".to_owned()
                        }
                        Err(e) => format!("refused: {e}"),
                    };
                    (c.seq, outcome)
                })
                .collect(),
        )),
    };
    let _ = job.reply.send(result.map_err(|e| e.to_string()));
    changed
}

fn ask(submit: &Submit, work: Work) -> Result<Done, String> {
    let (tx, rx) = mpsc::channel();
    if !submit(Job { work, reply: tx }) {
        return Err("the database stopped".into());
    }
    rx.recv().map_err(|_| "the database stopped".to_owned())?
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .build()
        .into()
}
/// The JSON answer, or the server's error message.
fn answer(
    response: Result<ureq::http::Response<ureq::Body>, ureq::Error>,
) -> Result<Value, String> {
    let mut response = response.map_err(|e| format!("cannot reach the cloud: {e}"))?;
    let status = response.status().as_u16();
    let body: Value = response.body_mut().read_json().unwrap_or(Value::Null);
    match status {
        200..=299 => Ok(body),
        401 => Err("this shop is no longer linked (sign in on the cloud and link again)".into()),
        _ => Err(body["error"]["message"]
            .as_str()
            .map_or_else(|| format!("cloud error {status}"), str::to_owned)),
    }
}

/// Only HTTPS, except to this computer (local development server).
pub fn normalize_server(raw: &str) -> Result<String, String> {
    let server = raw.trim().trim_end_matches('/').to_owned();
    let local = ["http://127.0.0.1", "http://localhost", "http://[::1]"]
        .iter()
        .any(|p| server.starts_with(p));
    if server.starts_with("https://") || local {
        Ok(server)
    } else {
        Err("the cloud address must start with https://".into())
    }
}

/// Links this main register with a code from the dashboard.
pub fn link(server: &str, code: &str, name: &str) -> Result<CloudFile, String> {
    let server = normalize_server(server)?;
    let body = answer(
        agent()
            .post(format!("{server}/api/v1/hub/link"))
            .send_json(json!({ "code": code.trim(), "name": name })),
    )?;
    let text = |k: &str| {
        body[k]
            .as_str()
            .map(str::to_owned)
            .ok_or("unexpected answer")
    };
    let uuid = |k: &str| -> Result<Uuid, String> {
        Uuid::parse_str(&text(k)?).map_err(|_| "unexpected answer".to_owned())
    };
    Ok(CloudFile {
        server,
        hub: uuid("hub")?,
        token: text("token")?,
        shop: uuid("shop")?,
        shop_name: text("shop_name")?,
        backup_salt: None,
        backup_key: None,
        last_backup: None,
    })
}

#[derive(Clone, Debug, Default)]
pub struct Status {
    pub syncing: bool,
    /// Records waiting to upload (after the last sync).
    pub pending: Option<u64>,
    pub last_sync: Option<i64>,
    pub error: Option<String>,
    /// Owner edits applied since the app started.
    pub applied: u64,
    pub last_backup: Option<i64>,
    pub backup_error: Option<String>,
    pub backing_up: bool,
}

/// Backups go up once a day (and on "Back up now").
const BACKUP_EVERY: i64 = 86_400;
/// Seals a fresh recovery bundle and uploads it; returns when it was made.
fn backup(file: &CloudFile, submit: &Submit, data: &Path) -> Result<i64, String> {
    let key = file.key().ok_or("set a backup password first")?;
    let dest = std::env::temp_dir().join(format!(
        "pos-heart-cloud-{}.posbackup",
        Uuid::new_v4().simple()
    ));
    ask(submit, Work::Snapshot(data.to_path_buf(), dest.clone()))?;
    let sealed = crate::cloud_backup::seal(&key, &dest);
    let _ = std::fs::remove_dir_all(&dest);
    let sealed = sealed.map_err(|e| e.to_string())?;
    answer(
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(600)))
            .http_status_as_error(false)
            .build()
            .new_agent()
            .post(file.url("/hub/backups"))
            .header("Authorization", &file.auth())
            .header("Content-Type", "application/octet-stream")
            .send(&sealed[..]),
    )?;
    let at = time::OffsetDateTime::now_utc().unix_timestamp();
    // Keep the time across restarts (cloud.json may have changed meanwhile).
    if let Some(mut saved) = CloudFile::load(data)
        && saved.hub == file.hub
    {
        saved.last_backup = Some(at);
        let _ = saved.save(data);
    }
    Ok(at)
}

/// One full sync: upload until the outbox is empty, then apply edits (and
/// upload what they changed, a few rounds at most).
fn sync(file: &CloudFile, submit: &Submit, status: &Mutex<Status>) -> Result<(), String> {
    for _ in 0..3 {
        if !round(file, submit, status)? {
            break;
        }
    }
    Ok(())
}
/// True when owner edits were applied (their records are now queued).
fn round(file: &CloudFile, submit: &Submit, status: &Mutex<Status>) -> Result<bool, String> {
    let agent = agent();
    loop {
        let Done::Batch(items) = ask(submit, Work::Batch(BATCH))? else {
            return Err("unexpected database answer".into());
        };
        if items.is_empty() {
            break;
        }
        let full = items.len() >= BATCH as usize;
        let body = answer(
            agent
                .post(file.url("/hub/upload"))
                .header("Authorization", &file.auth())
                .send_json(json!({ "items": items })),
        )?;
        let acked: Vec<i64> =
            serde_json::from_value(body["acked"].clone()).map_err(|_| "unexpected answer")?;
        ask(submit, Work::Ack(acked))?;
        if !full {
            break;
        }
    }
    let changes: Vec<Change> = serde_json::from_value(answer(
        agent
            .get(file.url("/hub/changes"))
            .header("Authorization", &file.auth())
            .call(),
    )?)
    .map_err(|_| "unexpected answer")?;
    if !changes.is_empty() {
        let Done::Applied(results) = ask(submit, Work::Apply(changes))? else {
            return Err("unexpected database answer".into());
        };
        let applied = results.iter().filter(|r| r.1 == "applied").count() as u64;
        answer(
            agent
                .post(file.url("/hub/changes/ack"))
                .header("Authorization", &file.auth())
                .send_json(json!({
                    "results": results
                        .iter()
                        .map(|(seq, outcome)| json!({ "seq": seq, "outcome": outcome }))
                        .collect::<Vec<_>>()
                })),
        )?;
        if let Ok(mut s) = status.lock() {
            s.applied += applied;
        }
        return Ok(true);
    }
    Ok(false)
}

/// The sync thread. Dropping it stops the loop after the current sync.
pub struct Syncer {
    stop: Arc<AtomicBool>,
    backup_now: Arc<AtomicBool>,
    wake: mpsc::Sender<()>,
    pub status: Arc<Mutex<Status>>,
}
impl Syncer {
    pub fn start(
        file: CloudFile,
        submit: Submit,
        data: std::path::PathBuf,
        repaint: impl Fn() + Send + 'static,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let backup_now = Arc::new(AtomicBool::new(false));
        let status = Arc::new(Mutex::new(Status {
            last_backup: file.last_backup,
            ..Status::default()
        }));
        let (wake, woken) = mpsc::channel::<()>();
        let (thread_stop, thread_status, thread_backup) =
            (stop.clone(), status.clone(), backup_now.clone());
        let _ = thread::Builder::new()
            .name("pos-cloud-sync".into())
            .spawn(move || {
                // A failed backup waits an hour before the schedule retries.
                let mut backup_after = 0;
                while !thread_stop.load(Ordering::SeqCst) {
                    if let Ok(mut s) = thread_status.lock() {
                        s.syncing = true;
                    }
                    repaint();
                    let result = sync(&file, &submit, &thread_status);
                    let pending = match ask(&submit, Work::Pending) {
                        Ok(Done::Pending(n)) => Some(n),
                        _ => None,
                    };
                    let failed = result.is_err();
                    if let Ok(mut s) = thread_status.lock() {
                        s.syncing = false;
                        s.pending = pending;
                        match result {
                            Ok(()) => {
                                s.error = None;
                                s.last_sync =
                                    Some(time::OffsetDateTime::now_utc().unix_timestamp());
                            }
                            Err(e) => s.error = Some(e),
                        }
                    }
                    repaint();
                    let now = time::OffsetDateTime::now_utc().unix_timestamp();
                    let last = thread_status.lock().ok().and_then(|s| s.last_backup);
                    let due = now >= backup_after && last.is_none_or(|at| now - at >= BACKUP_EVERY);
                    if !failed
                        && file.key().is_some()
                        && (thread_backup.swap(false, Ordering::SeqCst) || due)
                    {
                        if let Ok(mut s) = thread_status.lock() {
                            s.backing_up = true;
                        }
                        repaint();
                        let result = backup(&file, &submit, &data);
                        if let Ok(mut s) = thread_status.lock() {
                            s.backing_up = false;
                            match result {
                                Ok(at) => {
                                    s.last_backup = Some(at);
                                    s.backup_error = None;
                                }
                                Err(e) => {
                                    s.backup_error = Some(e);
                                    backup_after = now + 3600;
                                }
                            }
                        }
                        repaint();
                    }
                    match woken.recv_timeout(if failed { RETRY } else { IDLE }) {
                        Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            });
        Self {
            stop,
            backup_now,
            wake,
            status,
        }
    }
    /// Backs up after the next sync, whatever the schedule.
    pub fn backup_now(&self) {
        self.backup_now.store(true, Ordering::SeqCst);
        self.wake();
    }
    /// Syncs now (after a sale, or "Sync now").
    pub fn wake(&self) {
        let _ = self.wake.send(());
    }
    pub fn status(&self) -> Status {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }
}
impl Drop for Syncer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = self.wake.send(());
    }
}
