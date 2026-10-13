//! Explicitly enabled device maintenance. Heavy I/O runs only on the database worker.
use crate::backup;
use pos_core::Pos;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

const CONFIG: &str = "auto-backup.json";
const FOLDER: &str = "automatic-backups";
const INDEX: &str = "index.json";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub version: u32,
    pub enabled: bool,
    pub daily: bool,
    pub after_shift: bool,
    pub keep: u8,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            enabled: false,
            daily: true,
            after_shift: true,
            keep: 7,
        }
    }
}
fn invalid(message: &str) -> io::Error {
    io::Error::other(message)
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> io::Result<Option<T>> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    if !meta.is_file() || meta.len() > 1024 * 1024 {
        return Err(invalid("Maintenance file is not a bounded regular file"));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err(invalid("Maintenance file is too large"));
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(io::Error::other)
}
fn atomic_json<T: Serialize>(dir: &Path, name: &str, value: &T) -> io::Result<()> {
    let temporary = dir.join(format!(".maintenance-{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let bytes = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
        fs::rename(&temporary, dir.join(name))?;
        #[cfg(unix)]
        fs::File::open(dir)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
impl Settings {
    pub fn validate(&self) -> io::Result<()> {
        if self.version != 1 {
            return Err(invalid("Unsupported automatic backup settings version"));
        }
        if !(3..=30).contains(&self.keep) {
            return Err(invalid("Keep between 3 and 30 automatic backups"));
        }
        if self.enabled && !self.daily && !self.after_shift {
            return Err(invalid("Choose a backup schedule"));
        }
        Ok(())
    }
    pub fn load(data: &Path) -> io::Result<Self> {
        let settings: Self = read_json(&data.join(CONFIG))?.unwrap_or_default();
        settings.validate()?;
        Ok(settings)
    }
    /// Called only after Settings authorization on the worker. Never repairs unreadable files.
    pub fn save(&self, data: &Path) -> io::Result<()> {
        self.validate()?;
        Self::load(data)?;
        atomic_json(data, CONFIG, self)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    Daily,
    ShiftClosed(Uuid),
    ManualRetry,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub path: PathBuf,
    pub created_at: i64,
}
#[derive(Default, Debug)]
pub struct Outcome {
    pub created: Option<PathBuf>,
    pub warning: Option<String>,
    /// Most recently committed, verified automatic snapshot, including when the schedule is disabled or a no-op.
    pub latest: Option<Snapshot>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    name: String,
    day: String,
    shift: Option<Uuid>,
    created_at: i64,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Index {
    version: u32,
    saved: Vec<Saved>,
}
impl Default for Index {
    fn default() -> Self {
        Self {
            version: 1,
            saved: Vec::new(),
        }
    }
}
fn auto_name(name: &str) -> bool {
    name.strip_prefix("auto-")
        .and_then(|s| s.strip_suffix(".posbackup"))
        .is_some_and(|s| Uuid::parse_str(s).is_ok())
}
fn load_index(dir: &Path) -> io::Result<Index> {
    let index: Index = read_json(&dir.join(INDEX))?.unwrap_or_default();
    let mut names = std::collections::HashSet::new();
    if index.version != 1
        || index.saved.iter().any(|s| {
            !auto_name(&s.name)
                || !names.insert(&s.name)
                || time::Date::parse(
                    &s.day,
                    time::macros::format_description!("[year]-[month]-[day]"),
                )
                .is_err()
        })
    {
        return Err(invalid("Unsupported or invalid automatic backup index"));
    }
    Ok(index)
}
fn verified(dir: &Path, saved: &Saved) -> bool {
    backup::inspect(&dir.join(&saved.name)).is_ok_and(|r| r.manifest.created_at == saved.created_at)
}
/// Default-off policy is reloaded from the persisted, explicitly authorized device configuration.
/// Runs on the database worker and snapshots the last autosaved database/cart state.
/// Scheduled maintenance does not synchronously flush edits currently held in the UI.
pub fn run(pos: &mut Pos, data: &Path, trigger: Trigger, now: i64) -> pos_core::Result<Outcome> {
    let settings = Settings::load(data)?;
    if !settings.enabled {
        // Disabling the schedule does not erase previous backup evidence. This path
        // never creates folders or repairs malformed/missing maintenance metadata.
        let dir = data.join(FOLDER);
        let latest = fs::symlink_metadata(&dir)
            .ok()
            .filter(|m| m.is_dir())
            .and_then(|_| load_index(&dir).ok())
            .and_then(|index| {
                index
                    .saved
                    .iter()
                    .rev()
                    .find(|s| verified(&dir, s))
                    .map(|s| Snapshot {
                        path: dir.join(&s.name),
                        created_at: s.created_at,
                    })
            });
        return Ok(Outcome {
            latest,
            ..Default::default()
        });
    }
    let scheduled = (trigger != Trigger::Daily || settings.daily)
        && (!matches!(trigger, Trigger::ShiftClosed(_)) || settings.after_shift);
    let date = time::OffsetDateTime::from_unix_timestamp(now)
        .map_err(|_| invalid("Invalid backup time"))?
        .to_offset(time::macros::offset!(+7))
        .date()
        .to_string();
    let dir = data.join(FOLDER);
    match fs::symlink_metadata(&dir) {
        Ok(m) if !m.is_dir() => {
            return Err(invalid(
                "Automatic backup folder must be a local directory, not a symlink",
            )
            .into());
        }
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound && !scheduled => {
            return Ok(Outcome::default());
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => fs::create_dir(&dir)?,
        Err(e) => return Err(e.into()),
    }
    let mut index = load_index(&dir)?;
    let verified_saved: Vec<_> = index.saved.iter().filter(|s| verified(&dir, s)).collect();
    let latest = verified_saved.last().map(|s| Snapshot {
        path: dir.join(&s.name),
        created_at: s.created_at,
    });
    if !scheduled {
        return Ok(Outcome {
            latest,
            ..Default::default()
        });
    }
    let shift = match trigger {
        Trigger::ShiftClosed(id) => Some(id),
        _ => None,
    };
    if trigger != Trigger::ManualRetry
        && verified_saved.iter().any(|s| {
            if let Some(id) = shift {
                s.shift == Some(id)
            } else {
                s.day == date
            }
        })
    {
        return Ok(Outcome {
            latest,
            ..Default::default()
        });
    }
    let name = format!("auto-{}.posbackup", Uuid::new_v4());
    let path = backup::create_maintenance(pos, data, &dir.join(&name))?;
    let review = backup::inspect(&path)?;
    index.saved.push(Saved {
        name,
        day: date,
        shift,
        created_at: review.manifest.created_at,
    });
    // Publish ownership before pruning. If this fails, the new valid orphan is preserved.
    atomic_json(&dir, INDEX, &index)?;
    let valid: Vec<_> = index
        .saved
        .iter()
        .filter(|s| verified(&dir, s))
        .cloned()
        .collect();
    // Commit order is authoritative even if the wall clock moves backwards.
    let remove = valid.len().saturating_sub(usize::from(settings.keep));
    let mut warning = None;
    for old in valid.into_iter().take(remove) {
        // Recheck immediately before removal; damaged or foreign contents are never removed.
        if !verified(&dir, &old) {
            continue;
        }
        match fs::remove_dir_all(dir.join(&old.name)) {
            Ok(()) => index.saved.retain(|s| s.name != old.name),
            Err(e) => {
                warning = Some(format!(
                    "สำรองแล้ว แต่ลบชุดเก่าไม่สำเร็จ / Backup saved; old backup retained: {e}"
                ))
            }
        }
    }
    if let Err(e) = atomic_json(&dir, INDEX, &index) {
        warning = Some(format!(
            "สำรองแล้ว แต่บันทึกประวัติการเก็บไม่สำเร็จ / Backup saved; retention index update failed: {e}"
        ));
    }
    Ok(Outcome {
        latest: Some(Snapshot {
            path: path.clone(),
            created_at: review.manifest.created_at,
        }),
        created: Some(path),
        warning,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    type Result = std::result::Result<(), Box<dyn std::error::Error>>;
    fn setup() -> std::result::Result<(tempfile::TempDir, Pos), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let pos = Pos::open(dir.path().join("pos-heart.sqlite3"))?;
        Settings {
            enabled: true,
            keep: 3,
            ..Default::default()
        }
        .save(dir.path())?;
        Ok((dir, pos))
    }
    #[test]
    fn default_disabled_and_bad_or_future_config_preserved() -> Result {
        let dir = tempfile::tempdir()?;
        let mut pos = Pos::open(dir.path().join("pos-heart.sqlite3"))?;
        assert!(!Settings::load(dir.path())?.enabled);
        assert!(
            run(&mut pos, dir.path(), Trigger::Daily, 0)?
                .created
                .is_none()
        );
        assert!(!dir.path().join(FOLDER).exists());
        for bytes in [
            b"broken".to_vec(),
            serde_json::to_vec(&Settings {
                version: 2,
                ..Default::default()
            })?,
        ] {
            fs::write(dir.path().join(CONFIG), &bytes)?;
            assert!(Settings::load(dir.path()).is_err());
            assert!(Settings::default().save(dir.path()).is_err());
            assert_eq!(fs::read(dir.path().join(CONFIG))?, bytes);
        }
        Ok(())
    }
    #[test]
    fn bangkok_day_and_shift_retry_are_restart_safe() -> Result {
        let (dir, mut pos) = setup()?;
        // 16:59:59 UTC and 17:00 UTC straddle Bangkok midnight.
        let first = run(&mut pos, dir.path(), Trigger::Daily, 61_199)?;
        assert!(first.created.is_some());
        assert_eq!(
            first.latest.as_ref().map(|s| &s.path),
            first.created.as_ref()
        );
        assert!(
            run(&mut pos, dir.path(), Trigger::Daily, 61_199)?
                .created
                .is_none()
        );
        let second = run(&mut pos, dir.path(), Trigger::Daily, 61_200)?;
        assert!(second.created.is_some());
        let shift = Uuid::new_v4();
        assert!(
            run(&mut pos, dir.path(), Trigger::ShiftClosed(shift), 61_201)?
                .created
                .is_some()
        );
        drop(pos);
        let mut pos = Pos::open(dir.path().join("pos-heart.sqlite3"))?;
        assert!(
            run(&mut pos, dir.path(), Trigger::ShiftClosed(shift), 61_202)?
                .created
                .is_none()
        );
        let noop = run(&mut pos, dir.path(), Trigger::Daily, 61_202)?;
        assert!(noop.created.is_none());
        let latest = noop
            .latest
            .ok_or("verified backup status missing after restart")?;
        assert_eq!(
            backup::inspect(&latest.path)?.manifest.created_at,
            latest.created_at
        );
        Settings {
            enabled: true,
            daily: false,
            after_shift: true,
            ..Default::default()
        }
        .save(dir.path())?;
        let unscheduled = run(&mut pos, dir.path(), Trigger::Daily, 61_202)?;
        assert!(unscheduled.created.is_none());
        assert_eq!(unscheduled.latest, Some(latest));
        Ok(())
    }
    #[test]
    fn disabled_policy_retains_verified_status_without_writing_or_repairing_index() -> Result {
        let (dir, mut pos) = setup()?;
        let created = run(&mut pos, dir.path(), Trigger::Daily, 1)?;
        let latest = created.latest.ok_or("missing new snapshot")?;
        Settings::default().save(dir.path())?;
        let index_path = dir.path().join(FOLDER).join(INDEX);
        let original = fs::read(&index_path)?;
        let disabled = run(&mut pos, dir.path(), Trigger::Daily, i64::MAX)?;
        assert!(disabled.created.is_none());
        assert_eq!(disabled.latest, Some(latest.clone()));
        assert_eq!(fs::read(&index_path)?, original);
        fs::write(&index_path, "broken")?;
        let bad_index = run(&mut pos, dir.path(), Trigger::ManualRetry, 2)?;
        assert!(bad_index.created.is_none());
        assert!(bad_index.latest.is_none());
        assert_eq!(fs::read(&index_path)?, b"broken");
        fs::write(&index_path, original)?;
        fs::write(latest.path.join("manifest.json"), "broken")?;
        assert!(
            run(&mut pos, dir.path(), Trigger::Daily, 3)?
                .latest
                .is_none()
        );
        assert!(latest.path.exists());
        Ok(())
    }
    #[test]
    fn retention_only_prunes_owned_valid_snapshots_after_success() -> Result {
        let (dir, mut pos) = setup()?;
        let data = dir.path();
        let parent = data.join(FOLDER);
        let first = run(&mut pos, data, Trigger::ManualRetry, 1)?
            .created
            .ok_or("missing first")?;
        let damaged = run(&mut pos, data, Trigger::ManualRetry, 2)?
            .created
            .ok_or("missing second")?;
        fs::write(damaged.join("manifest.json"), "broken")?;
        let manual = parent.join("manual.posbackup");
        backup::create_maintenance(&mut pos, data, &manual)?;
        let orphan = parent.join(format!("auto-{}.posbackup", Uuid::new_v4()));
        backup::create_maintenance(&mut pos, data, &orphan)?;
        let unknown = parent.join("unknown");
        fs::create_dir(&unknown)?;
        fs::write(unknown.join("keep"), "untouched")?;
        for n in 3..=6 {
            assert!(
                run(&mut pos, data, Trigger::ManualRetry, n)?
                    .created
                    .is_some()
            );
        }
        assert!(!first.exists());
        assert!(damaged.exists());
        assert!(manual.exists());
        assert!(orphan.exists());
        assert!(unknown.join("keep").exists());
        let index = load_index(&parent)?;
        assert_eq!(
            index.saved.iter().filter(|s| verified(&parent, s)).count(),
            3
        );
        // Corrupt current business sidecar: no new snapshot, no pruning of any old folder.
        let before: Vec<_> = fs::read_dir(&parent)?
            .map(|e| e.map(|e| e.file_name()))
            .collect::<std::result::Result<_, _>>()?;
        fs::write(data.join("shop.json"), "broken")?;
        assert!(run(&mut pos, data, Trigger::ManualRetry, 7).is_err());
        let after: Vec<_> = fs::read_dir(&parent)?
            .map(|e| e.map(|e| e.file_name()))
            .collect::<std::result::Result<_, _>>()?;
        assert_eq!(before.len(), after.len());
        Ok(())
    }
    #[test]
    fn damaged_latest_and_bad_index_do_not_claim_success() -> Result {
        let (dir, mut pos) = setup()?;
        let path = run(&mut pos, dir.path(), Trigger::Daily, 1)?
            .created
            .ok_or("no backup")?;
        fs::write(path.join("pos-heart.sqlite3"), "broken")?;
        assert!(
            run(&mut pos, dir.path(), Trigger::Daily, 2)?
                .created
                .is_some()
        );
        let index = dir.path().join(FOLDER).join(INDEX);
        let bytes = b"{\"version\":999,\"saved\":[]}";
        fs::write(&index, bytes)?;
        assert!(run(&mut pos, dir.path(), Trigger::ManualRetry, 3).is_err());
        assert_eq!(fs::read(index)?, bytes);
        assert!(path.exists());
        Ok(())
    }
    #[test]
    fn enabled_maintenance_works_when_cashier_is_locked_but_manual_stays_guarded() -> Result {
        use pos_domain::StaffInput;
        let (dir, mut pos) = setup()?;
        pos.save_staff(
            &StaffInput {
                id: None,
                name: "Owner".into(),
                role: Some(pos_domain::OWNER.into()),
                pin: Some("9999".into()),
                active: true,
            },
            1,
        )?;
        pos.sign_out();
        assert!(backup::create(&mut pos, dir.path(), &dir.path().join("manual")).is_err());
        assert!(
            run(&mut pos, dir.path(), Trigger::Daily, 1)?
                .created
                .is_some()
        );
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn linked_destination_refuses_without_touching_external_folder() -> Result {
        let (dir, mut pos) = setup()?;
        let external = tempfile::tempdir()?;
        std::os::unix::fs::symlink(external.path(), dir.path().join(FOLDER))?;
        assert!(run(&mut pos, dir.path(), Trigger::Daily, 1).is_err());
        assert_eq!(fs::read_dir(external.path())?.count(), 0);
        Ok(())
    }
}
