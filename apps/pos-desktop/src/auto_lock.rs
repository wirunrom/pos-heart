//! Device-local idle lock policy. Only native input advances the monotonic timer.
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::Path,
    time::{Duration, Instant},
};
use uuid::Uuid;

const FILE: &str = "auto-lock.json";
pub const TIMEOUTS: [u32; 6] = [0, 1, 5, 10, 15, 30];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub version: u32,
    /// Zero disables the timer. No staff means the timer is inactive.
    pub timeout_minutes: u32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            timeout_minutes: 0,
        }
    }
}
impl Settings {
    fn validate(&self) -> io::Result<()> {
        if self.version != 1 || !TIMEOUTS.contains(&self.timeout_minutes) {
            return Err(io::Error::other(
                "Unsupported auto-lock settings version or timeout",
            ));
        }
        Ok(())
    }
    fn load(dir: &Path) -> io::Result<Self> {
        let bytes = match fs::read(dir.join(FILE)) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error),
        };
        let settings: Self = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
        settings.validate()?;
        Ok(settings)
    }
    fn save(&self, dir: &Path) -> io::Result<()> {
        self.validate()?;
        // Refuse an unreadable/future file even if it changed after launch.
        Self::load(dir)?;
        let bytes = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        let temp = dir.join(format!(".auto-lock-{}.tmp", Uuid::new_v4()));
        let result = (|| {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&temp, dir.join(FILE))
        })();
        if result.is_err() {
            let _ = fs::remove_file(temp);
        }
        result
    }
}

pub struct State {
    pub settings: Settings,
    /// A failed load freezes configuration until the file is repaired and the app restarts.
    pub readonly: bool,
    pub error: Option<String>,
    last_activity: Option<Instant>,
    triggered: bool,
}
impl State {
    pub fn load(dir: &Path) -> Self {
        match Settings::load(dir) {
            Ok(settings) => Self {
                settings,
                readonly: false,
                error: None,
                last_activity: None,
                triggered: false,
            },
            Err(error) => Self {
                settings: Settings::default(),
                readonly: true,
                error: Some(error.to_string()),
                last_activity: None,
                triggered: false,
            },
        }
    }
    pub fn set_timeout(&mut self, dir: &Path, minutes: u32) -> io::Result<()> {
        if self.readonly {
            return Err(io::Error::other("Auto-lock settings are read-only"));
        }
        let settings = Settings {
            version: 1,
            timeout_minutes: minutes,
        };
        settings.save(dir)?;
        self.settings = settings;
        self.error = None;
        self.reset(Instant::now());
        Ok(())
    }
    pub fn reset(&mut self, now: Instant) {
        self.last_activity = Some(now);
        self.triggered = false;
    }
    /// Poll before handling input in every viewport. An event arriving after
    /// the deadline locks first; it cannot revive an expired session.
    pub fn poll(
        &mut self,
        now: Instant,
        staff_enabled: bool,
        signed_in: bool,
        events: &[egui::Event],
    ) -> bool {
        if !staff_enabled || !signed_in || self.settings.timeout_minutes == 0 {
            self.last_activity = None;
            self.triggered = false;
            return false;
        }
        if self.triggered {
            return false;
        }
        let Some(last) = self.last_activity else {
            self.reset(now);
            return false;
        };
        let timeout = Duration::from_secs(u64::from(self.settings.timeout_minutes) * 60);
        if now.saturating_duration_since(last) >= timeout {
            self.triggered = true;
            return true;
        }
        if events.iter().any(is_user_input) {
            self.last_activity = Some(now);
        }
        false
    }
    pub fn remaining(&self, now: Instant) -> Option<Duration> {
        if self.triggered || self.settings.timeout_minutes == 0 {
            return None;
        }
        let last = self.last_activity?;
        let timeout = Duration::from_secs(u64::from(self.settings.timeout_minutes) * 60);
        Some(timeout.saturating_sub(now.saturating_duration_since(last)))
    }
}

/// Repaints, viewport focus, screenshots and worker completion are not activity.
pub fn is_user_input(event: &egui::Event) -> bool {
    matches!(
        event,
        egui::Event::Copy
            | egui::Event::Cut
            | egui::Event::Paste(_)
            | egui::Event::Text(_)
            | egui::Event::Key { .. }
            | egui::Event::ModifiersChanged(_)
            | egui::Event::PointerMoved(_)
            | egui::Event::MouseMoved(_)
            | egui::Event::PointerButton { .. }
            | egui::Event::MouseWheel { .. }
            | egui::Event::Touch { .. }
            | egui::Event::Zoom(_)
            | egui::Event::Rotate(_)
            | egui::Event::Ime(
                egui::ImeEvent::Preedit { .. }
                    | egui::ImeEvent::Commit(_)
                    | egui::ImeEvent::DeleteSurrounding { .. }
            )
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quiet_repaints_do_not_reset_and_late_input_cannot_bypass_deadline() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let mut state = State::load(dir.path());
        state.set_timeout(dir.path(), 1)?;
        let start = Instant::now();
        state.reset(start);
        for seconds in [1, 15, 30, 59] {
            assert!(!state.poll(
                start + Duration::from_secs(seconds),
                true,
                true,
                &[egui::Event::WindowFocused(true)]
            ));
        }
        assert!(state.poll(
            start + Duration::from_secs(60),
            true,
            true,
            &[egui::Event::Text("1".into())]
        ));
        assert!(!state.poll(start + Duration::from_secs(61), true, true, &[]));
        assert_eq!(state.remaining(start + Duration::from_secs(61)), None);
        Ok(())
    }
    #[test]
    fn native_activity_extends_deadline_but_disabled_and_no_staff_never_lock() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let mut state = State::load(dir.path());
        state.set_timeout(dir.path(), 1)?;
        let start = Instant::now();
        state.reset(start);
        assert!(!state.poll(
            start + Duration::from_secs(50),
            true,
            true,
            &[egui::Event::PointerMoved(egui::pos2(2.0, 3.0))]
        ));
        assert_eq!(
            state.remaining(start + Duration::from_secs(60)),
            Some(Duration::from_secs(50))
        );
        assert!(!state.poll(
            start + Duration::from_secs(100),
            true,
            true,
            &[egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE
            }]
        ));
        assert!(state.poll(start + Duration::from_secs(160), true, true, &[]));
        state.reset(start);
        assert!(!state.poll(start + Duration::from_secs(900), false, true, &[]));
        assert!(!state.poll(start + Duration::from_secs(900), true, false, &[]));
        state.set_timeout(dir.path(), 0)?;
        state.reset(start);
        assert!(!state.poll(start + Duration::from_secs(900), true, true, &[]));
        Ok(())
    }
    #[test]
    fn file_roundtrip_and_corrupt_future_or_external_changes_are_never_overwritten()
    -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let mut state = State::load(dir.path());
        assert_eq!(state.settings.timeout_minutes, 0);
        state.set_timeout(dir.path(), 5)?;
        assert_eq!(State::load(dir.path()).settings.timeout_minutes, 5);
        let original = fs::read(dir.path().join(FILE))?;
        assert!(state.set_timeout(dir.path(), 2).is_err());
        assert_eq!(fs::read(dir.path().join(FILE))?, original);
        for bad in [
            r#"{"version":2,"timeout_minutes":5}"#,
            r#"{"version":1,"timeout_minutes":2}"#,
            "broken",
        ] {
            fs::write(dir.path().join(FILE), bad)?;
            let mut blocked = State::load(dir.path());
            assert!(blocked.readonly);
            assert!(blocked.set_timeout(dir.path(), 10).is_err());
            assert!(state.set_timeout(dir.path(), 10).is_err());
            assert_eq!(fs::read(dir.path().join(FILE))?, bad.as_bytes());
        }
        Ok(())
    }
    #[test]
    fn a_new_session_starts_a_fresh_deadline() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let mut state = State::load(dir.path());
        state.set_timeout(dir.path(), 1)?;
        let start = Instant::now();
        state.reset(start);
        assert!(state.poll(start + Duration::from_secs(60), true, true, &[]));
        assert!(!state.poll(start + Duration::from_secs(70), true, false, &[]));
        assert!(!state.poll(start + Duration::from_secs(80), true, true, &[]));
        assert!(!state.poll(start + Duration::from_secs(139), true, true, &[]));
        assert!(state.poll(start + Duration::from_secs(140), true, true, &[]));
        Ok(())
    }
}
