//! Shop-chosen layout of printed documents: header and footer text, logo,
//! text size, language and which details to show. Stored in shop.json, so it
//! travels with backups; legal content of tax invoices is never optional.
use super::render::{Bitmap, TextSize};
use crate::i18n::Language;
use serde::{Deserialize, Serialize};
use std::{io, path::Path};

/// Logo file beside the database (1-bit PNG, already dithered).
pub const LOGO_FILE: &str = "receipt-logo.png";
/// Widest logo: 70% of 58 mm paper, so it fits both widths.
const LOGO_MAX_W: u32 = 268;
const LOGO_MAX_H: u32 = 160;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocLanguage {
    /// Follow the app's language.
    #[default]
    App,
    Thai,
    English,
}
impl DocLanguage {
    pub fn resolve(self, app: Language) -> Language {
        match self {
            Self::App => app,
            Self::Thai => Language::Thai,
            Self::English => Language::English,
        }
    }
}
fn yes() -> bool {
    true
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReceiptTemplate {
    /// Replaces "ใบเสร็จรับเงิน" when set (not on tax invoices).
    pub title: String,
    /// Lines under the shop name: address, phone, LINE…
    pub header: String,
    /// Lines at the end: thank-you, return policy, Wi-Fi…
    pub footer: String,
    pub logo: bool,
    pub size: TextSize,
    pub language: DocLanguage,
    #[serde(default = "yes")]
    pub show_unit_price: bool,
    #[serde(default = "yes")]
    pub show_options: bool,
    #[serde(default = "yes")]
    pub show_notes: bool,
    pub show_sku: bool,
    #[serde(default = "yes")]
    pub show_staff: bool,
    #[serde(default = "yes")]
    pub show_label: bool,
    /// Cash received and change.
    #[serde(default = "yes")]
    pub show_cash: bool,
    /// PromptPay QR with the amount on check bills (needs a PromptPay ID).
    #[serde(default = "yes")]
    pub bill_qr: bool,
}
impl Default for ReceiptTemplate {
    fn default() -> Self {
        Self {
            title: String::new(),
            header: String::new(),
            footer: String::new(),
            logo: false,
            size: TextSize::Normal,
            language: DocLanguage::App,
            show_unit_price: true,
            show_options: true,
            show_notes: true,
            show_sku: false,
            show_staff: true,
            show_label: true,
            show_cash: true,
            bill_qr: true,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct KitchenTemplate {
    pub size: TextSize,
    pub language: DocLanguage,
    #[serde(default = "yes")]
    pub show_options: bool,
    #[serde(default = "yes")]
    pub show_staff: bool,
    #[serde(default = "yes")]
    pub show_guests: bool,
}
impl Default for KitchenTemplate {
    fn default() -> Self {
        Self {
            size: TextSize::Large,
            language: DocLanguage::App,
            show_options: true,
            show_staff: true,
            show_guests: true,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Templates {
    pub receipt: ReceiptTemplate,
    pub kitchen: KitchenTemplate,
}
impl Templates {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        let r = &self.receipt;
        let block_ok = |s: &str| s.chars().count() <= 400 && s.lines().count() <= 8;
        if r.title.chars().count() > 60 || !block_ok(&r.header) || !block_ok(&r.footer) {
            return Err("template");
        }
        Ok(())
    }
}
/// Non-empty trimmed lines of a header/footer block.
pub fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines().map(str::trim).filter(|l| !l.is_empty())
}

/// Turns any PNG/JPEG into a dithered 1-bit logo and saves it beside the
/// database. Returns the bitmap for previews.
pub fn import_logo(source: &Path, dir: &Path) -> io::Result<Bitmap> {
    let img = image::open(source).map_err(io::Error::other)?;
    let (w, h) = (img.width().max(1), img.height().max(1));
    // Fit inside the box, keeping the aspect ratio.
    let k = (f64::from(LOGO_MAX_W) / f64::from(w)).min(f64::from(LOGO_MAX_H) / f64::from(h));
    let nw = ((f64::from(w) * k).round() as u32).max(1);
    let nh = ((f64::from(h) * k).round() as u32).max(1);
    // Transparent pixels count as paper (white).
    let rgba = img
        .resize_exact(nw, nh, image::imageops::FilterType::Triangle)
        .to_rgba8();
    let mut gray: Vec<f32> = rgba
        .pixels()
        .map(|p| {
            let [r, g, b, a] = p.0;
            let lum = 0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b);
            let alpha = f32::from(a) / 255.0;
            lum * alpha + 255.0 * (1.0 - alpha)
        })
        .collect();
    let bitmap = dither(&mut gray, nw, nh);
    let mut out = image::GrayImage::new(nw, nh);
    for (i, ink) in bitmap.ink.iter().enumerate() {
        let (x, y) = (i as u32 % nw, i as u32 / nw);
        out.put_pixel(x, y, image::Luma([if *ink { 0 } else { 255 }]));
    }
    out.save(dir.join(LOGO_FILE)).map_err(io::Error::other)?;
    Ok(bitmap)
}
/// Floyd–Steinberg dithering to 1 bit.
fn dither(gray: &mut [f32], w: u32, h: u32) -> Bitmap {
    let (w, h) = (w as usize, h as usize);
    let mut ink = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let old = gray[i];
            let new = if old < 128.0 { 0.0 } else { 255.0 };
            ink[i] = new == 0.0;
            let err = old - new;
            let mut spread = |dx: isize, dy: usize, f: f32| {
                let nx = x as isize + dx;
                let ny = y + dy;
                if nx >= 0 && (nx as usize) < w && ny < h {
                    gray[ny * w + nx as usize] += err * f;
                }
            };
            spread(1, 0, 7.0 / 16.0);
            spread(-1, 1, 3.0 / 16.0);
            spread(0, 1, 5.0 / 16.0);
            spread(1, 1, 1.0 / 16.0);
        }
    }
    Bitmap {
        width: w as u32,
        height: h as u32,
        ink,
    }
}
/// The saved logo, if any.
pub fn load_logo(dir: &Path) -> Option<Bitmap> {
    let img = image::open(dir.join(LOGO_FILE)).ok()?.to_luma8();
    let ink = img.pixels().map(|p| p.0[0] < 128).collect();
    Some(Bitmap {
        width: img.width(),
        height: img.height(),
        ink,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_shop_files_get_default_templates() -> Result<(), serde_json::Error> {
        let t: Templates = serde_json::from_str("{}")?;
        assert!(t.is_default());
        let partial: Templates = serde_json::from_str(r#"{"receipt":{"footer":"ขอบคุณ"}}"#)?;
        assert!(partial.receipt.show_unit_price && partial.receipt.footer == "ขอบคุณ");
        Ok(())
    }
    #[test]
    fn logos_are_resized_and_dithered() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let src = dir.path().join("logo.png");
        // A 600×200 left-to-right gradient.
        let img = image::GrayImage::from_fn(600, 200, |x, _| image::Luma([(x * 255 / 599) as u8]));
        img.save(&src)?;
        let logo = import_logo(&src, dir.path())?;
        assert!(logo.width <= LOGO_MAX_W && logo.height <= LOGO_MAX_H);
        // Dark on the left, light on the right.
        let left = logo
            .ink
            .iter()
            .step_by(logo.width as usize)
            .filter(|i| **i)
            .count();
        assert!(left > 0);
        assert_eq!(load_logo(dir.path()), Some(logo));
        let mut bad = Templates::default();
        bad.receipt.header = "a\n".repeat(9);
        assert!(bad.validate().is_err());
        Ok(())
    }
}
