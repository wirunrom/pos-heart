//! QR codes for PromptPay. Always dark modules on a white card with a
//! four-module quiet zone, in both themes, so every banking app can read it.
use super::tokens::{primitive, radius};
use eframe::egui::{CornerRadius, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use qrcodegen::{QrCode, QrCodeEcc};

const QUIET: i32 = 4;

/// Draws `payload` as a square QR code of side `size` points.
/// Returns None when the payload cannot be encoded.
pub fn show(ui: &mut Ui, payload: &str, size: f32) -> Option<Response> {
    let code = QrCode::encode_text(payload, QrCodeEcc::Medium).ok()?;
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect(
        rect,
        CornerRadius::same(radius::SM),
        primitive::STONE_0,
        Stroke::new(1.0, super::Theme::of(ui).p.border_default),
        StrokeKind::Inside,
    );
    let modules = code.size() + QUIET * 2;
    // Whole pixels per module keep edges crisp.
    let ppp = ui.ctx().pixels_per_point();
    let cell = ((size * ppp) / modules as f32).floor().max(1.0) / ppp;
    let origin = rect.center() - vec2(cell * modules as f32, cell * modules as f32) / 2.0;
    let origin = pos2(
        (origin.x * ppp).round() / ppp,
        (origin.y * ppp).round() / ppp,
    );
    for y in 0..code.size() {
        // Merge horizontal runs into one rectangle each.
        let mut x = 0;
        while x < code.size() {
            if !code.get_module(x, y) {
                x += 1;
                continue;
            }
            let start = x;
            while x < code.size() && code.get_module(x, y) {
                x += 1;
            }
            let min = origin + vec2((start + QUIET) as f32, (y + QUIET) as f32) * cell;
            painter.rect_filled(
                Rect::from_min_size(min, vec2((x - start) as f32 * cell, cell)),
                CornerRadius::ZERO,
                primitive::STONE_975,
            );
        }
    }
    Some(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn promptpay_payloads_fit_in_a_small_code() -> Result<(), Box<dyn std::error::Error>> {
        let id = pos_domain::PromptPayId::parse("1234567890123")?;
        let payload = id.payload(Some("999999.99".parse()?));
        let code =
            QrCode::encode_text(&payload, QrCodeEcc::Medium).map_err(|e| format!("{e:?}"))?;
        // Version 4 (33 modules) or smaller stays readable at 200 points.
        assert!(code.size() <= 33, "{}", code.size());
        Ok(())
    }
}
