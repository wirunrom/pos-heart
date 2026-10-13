//! Product pictures: import (fit within 256 px, PNG) and a small texture
//! cache. Pictures load in the background only when a tile or editor needs
//! them, and the least recently used textures are dropped beyond a limit,
//! so a catalog of thousands stays light.
use crate::{app::App, worker::Command};
use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use uuid::Uuid;

/// Longest side of a stored picture.
pub const MAX_SIDE: u32 = 256;
/// Textures kept at once (256² RGBA ≈ 256 KB each).
const CACHE: usize = 120;

#[derive(Default)]
pub struct ImageCache {
    textures: HashMap<Uuid, (i64, egui::TextureHandle, u64)>,
    pending: HashSet<(Uuid, i64)>,
    /// Revisions known to have no stored picture (e.g. removed meanwhile).
    missing: HashSet<(Uuid, i64)>,
    tick: u64,
}
/// Reads a PNG/JPEG, fits it within 256 px and encodes it as PNG.
pub fn import(path: &Path) -> Result<Vec<u8>, String> {
    let img = image::open(path).map_err(|e| e.to_string())?;
    let img = if img.width() > MAX_SIDE || img.height() > MAX_SIDE {
        img.resize(MAX_SIDE, MAX_SIDE, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let mut out = std::io::Cursor::new(Vec::new());
    img.to_rgba8()
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}
/// Paints `tex` to fill `rect` (cropping the longer side), with rounding.
pub fn paint_cover(
    painter: &egui::Painter,
    rect: egui::Rect,
    tex: (egui::TextureId, egui::Vec2),
    corner: egui::CornerRadius,
) {
    let (id, size) = tex;
    if size.x <= 0.0 || size.y <= 0.0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let want = rect.width() / rect.height();
    let have = size.x / size.y;
    let uv = if have > want {
        let w = want / have;
        egui::Rect::from_min_max(
            egui::pos2((1.0 - w) / 2.0, 0.0),
            egui::pos2((1.0 + w) / 2.0, 1.0),
        )
    } else {
        let h = have / want;
        egui::Rect::from_min_max(
            egui::pos2(0.0, (1.0 - h) / 2.0),
            egui::pos2(1.0, (1.0 + h) / 2.0),
        )
    };
    painter.add(
        egui::epaint::RectShape::filled(rect, corner, egui::Color32::WHITE).with_texture(id, uv),
    );
}
/// Image files a drop or a pasted path may name.
pub fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| ["png", "jpg", "jpeg"].contains(&e.to_ascii_lowercase().as_str()))
}
impl App {
    /// The product's picture texture, asking the worker for it when missing.
    pub fn product_texture(
        &mut self,
        id: Uuid,
        rev: Option<i64>,
    ) -> Option<(egui::TextureId, egui::Vec2)> {
        let rev = rev?;
        self.images.tick += 1;
        let tick = self.images.tick;
        if let Some((have, tex, used)) = self.images.textures.get_mut(&id)
            && *have == rev
        {
            *used = tick;
            return Some((tex.id(), tex.size_vec2()));
        }
        if !self.images.pending.contains(&(id, rev))
            && !self.images.missing.contains(&(id, rev))
            && self.send_background(Command::ProductImage(id))
        {
            self.images.pending.insert((id, rev));
        }
        // An older picture is fine until the new one arrives.
        self.images
            .textures
            .get(&id)
            .map(|(_, tex, _)| (tex.id(), tex.size_vec2()))
    }
    /// A loaded picture (or none) from the worker.
    pub fn image_loaded(&mut self, ctx: &egui::Context, id: Uuid, image: Option<(i64, Vec<u8>)>) {
        self.images.pending.retain(|(p, _)| *p != id);
        let Some((rev, png)) = image else {
            self.images.textures.remove(&id);
            if let Some(rev) = self.known.get(&id).and_then(|p| p.image) {
                self.images.missing.insert((id, rev));
            }
            return;
        };
        let Ok(decoded) = image::load_from_memory(&png) else {
            self.images.missing.insert((id, rev));
            return;
        };
        let rgba = decoded.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
        let tex = ctx.load_texture(format!("product-{id}"), color, egui::TextureOptions::LINEAR);
        self.images.tick += 1;
        self.images
            .textures
            .insert(id, (rev, tex, self.images.tick));
        if self.images.textures.len() > CACHE
            && let Some(oldest) = self
                .images
                .textures
                .iter()
                .min_by_key(|(_, (_, _, used))| *used)
                .map(|(id, _)| *id)
        {
            self.images.textures.remove(&oldest);
        }
    }
    /// Imports a file as the product's picture.
    pub fn set_picture_from(&mut self, id: Uuid, path: &Path) {
        match import(path) {
            Ok(png) if png.len() <= pos_core::MAX_IMAGE_BYTES => {
                self.send(Command::SetProductImage(id, Some(png)));
            }
            Ok(_) => {
                self.error = Some(
                    self.lang
                        .text("รูปใหญ่เกินไป", "The picture is too large.")
                        .into(),
                );
            }
            Err(e) => {
                self.error = Some(format!(
                    "{} ({e})",
                    self.lang.text(
                        "เปิดรูปไม่ได้ ใช้ไฟล์ PNG หรือ JPG",
                        "Cannot open the picture; use a PNG or JPG file"
                    )
                ));
            }
        }
    }
}
