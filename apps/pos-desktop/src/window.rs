//! Window sizing policy (design/docs/01 "หน้าจอปรับขนาด").
//! The window stays freely resizable, never below the 1000×700-point layout
//! minimum. Screens too small for that minimum (e.g. 1366×768 at 125% Windows
//! scaling) get a smaller UI zoom instead of a cut-off pay button. Cashier
//! devices can also run full screen.
use eframe::egui::{self, Vec2, vec2};

/// Smallest content size the layouts are designed and checked for.
pub const MIN_SIZE: Vec2 = vec2(1000.0, 700.0);
/// Room kept for a taskbar/dock, the title bar and window borders.
const RESERVE: Vec2 = vec2(24.0, 96.0);
/// Never shrink text below this; smaller screens are not supported.
const MIN_ZOOM: f32 = 0.6;

/// UI zoom that fits the minimum layout on a monitor of `native` points
/// (OS scaling applied, app zoom not). 1.0 when the screen is big enough;
/// otherwise rounded down to a 0.05 step so text stays crisp.
pub fn fit_zoom(native: Vec2) -> f32 {
    let fit = ((native.x - RESERVE.x) / MIN_SIZE.x)
        .min((native.y - RESERVE.y) / MIN_SIZE.y)
        .min(1.0);
    if fit >= 1.0 || !fit.is_finite() {
        return 1.0;
    }
    ((fit * 20.0).floor() / 20.0).max(MIN_ZOOM)
}

pub fn fullscreen_from_saved(value: &str) -> bool {
    value.trim() == "fullscreen"
}
pub fn fullscreen_saved(fullscreen: bool) -> &'static str {
    if fullscreen { "fullscreen" } else { "windowed" }
}

#[derive(Default)]
pub struct WindowFit {
    /// Zoom this module set; a user's own Ctrl +/− zoom is left alone.
    auto_zoom: Option<f32>,
    applied_fullscreen: Option<bool>,
}
impl WindowFit {
    /// Runs every frame: refits when the monitor changes and applies the
    /// full-screen preference when it changes.
    pub fn update(&mut self, ctx: &egui::Context, fullscreen: bool) {
        if self.applied_fullscreen != Some(fullscreen) {
            self.applied_fullscreen = Some(fullscreen);
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(fullscreen));
        }
        let Some(monitor) = ctx.input(|i| i.viewport().monitor_size) else {
            return;
        };
        let current = ctx.zoom_factor();
        // Monitor size is reported in UI points, which already include zoom.
        let native = monitor * current;
        let wanted = fit_zoom(native);
        let user_zoomed = self.auto_zoom.is_some_and(|z| (z - current).abs() > 0.001)
            || (self.auto_zoom.is_none() && (current - 1.0).abs() > 0.001);
        if user_zoomed || (wanted - current).abs() < 0.001 {
            return;
        }
        ctx.set_zoom_factor(wanted);
        self.auto_zoom = Some(wanted);
        // The minimum is in UI points; re-send it so the OS limit follows the zoom.
        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(MIN_SIZE));
        if wanted < 1.0 && !fullscreen {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn large_screens_keep_full_size() {
        assert_eq!(fit_zoom(vec2(1920.0, 1080.0)), 1.0);
        assert_eq!(fit_zoom(vec2(1280.0, 800.0)), 1.0);
        assert_eq!(fit_zoom(vec2(1024.0, 796.0)), 1.0);
    }
    #[test]
    fn small_or_scaled_screens_shrink_to_fit() {
        // 1366×768 at 125% Windows scaling is 1092.8×614.4 points.
        let z = fit_zoom(vec2(1092.8, 614.4));
        assert_eq!(z, 0.7);
        assert!(MIN_SIZE.y * z <= 614.4 - RESERVE.y);
        // 1024×768 at 100%.
        assert_eq!(fit_zoom(vec2(1024.0, 768.0)), 0.95);
        // Tiny screens stop at the readable floor.
        assert_eq!(fit_zoom(vec2(640.0, 480.0)), MIN_ZOOM);
    }
    #[test]
    fn fullscreen_preference_round_trips() {
        assert!(fullscreen_from_saved(fullscreen_saved(true)));
        assert!(!fullscreen_from_saved(fullscreen_saved(false)));
        assert!(!fullscreen_from_saved(""));
    }
}
