//! Lays out a receipt document and draws it into a grayscale bitmap at the
//! printer's resolution (203 dpi: 384 dots for 58 mm paper, 576 for 80 mm).
//! Thai is drawn as an image because most thermal printers lack Thai fonts.
use crate::ui::{text::break_lines, theme::font_bytes};
use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};
use image::{GrayImage, Luma};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Body,
    Small,
    Strong,
    Title,
    /// Large money total.
    Total,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
}
/// A 1-bit picture (logo) at printer dots; `true` is ink.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    pub ink: Vec<bool>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    /// Centred picture, e.g. the shop logo.
    Image(Bitmap),
    /// Centred QR code (e.g. PromptPay on a check bill).
    Qr(String),
    Text {
        text: String,
        style: Style,
        align: Align,
    },
    /// Text on the left (wraps), amount on the right (never wraps).
    Row {
        left: String,
        right: String,
        style: Style,
        indent: bool,
    },
    Divider,
    Space,
}
/// A printable document, built on the UI thread and rendered by the print job.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Doc {
    pub blocks: Vec<Block>,
    /// Text size chosen in the template.
    pub size: TextSize,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextSize {
    Small,
    #[default]
    Normal,
    Large,
}
impl TextSize {
    pub fn scale(self) -> f32 {
        match self {
            Self::Small => 0.85,
            Self::Normal => 1.0,
            Self::Large => 1.2,
        }
    }
}
impl Doc {
    pub fn image(&mut self, bitmap: Bitmap) {
        self.blocks.push(Block::Image(bitmap));
    }
    pub fn qr(&mut self, payload: impl Into<String>) {
        self.blocks.push(Block::Qr(payload.into()));
    }
    pub fn text(&mut self, text: impl Into<String>, style: Style, align: Align) {
        self.blocks.push(Block::Text {
            text: text.into(),
            style,
            align,
        });
    }
    pub fn row(&mut self, left: impl Into<String>, right: impl Into<String>, style: Style) {
        self.blocks.push(Block::Row {
            left: left.into(),
            right: right.into(),
            style,
            indent: false,
        });
    }
    /// A detail row under an item (options, unit price, discount).
    pub fn detail(&mut self, left: impl Into<String>, right: impl Into<String>) {
        self.blocks.push(Block::Row {
            left: left.into(),
            right: right.into(),
            style: Style::Small,
            indent: true,
        });
    }
    pub fn divider(&mut self) {
        self.blocks.push(Block::Divider);
    }
    pub fn space(&mut self) {
        self.blocks.push(Block::Space);
    }
}

struct Faces {
    sans: [FontRef<'static>; 3],
    strong: [FontRef<'static>; 3],
    mono: [FontRef<'static>; 3],
    mono_strong: [FontRef<'static>; 3],
}
fn faces() -> Option<Faces> {
    let f = |b: &'static [u8]| FontRef::try_from_slice(b).ok();
    let noto = || f(font_bytes::NOTO_THAI);
    Some(Faces {
        sans: [
            f(font_bytes::PLEX_SANS)?,
            f(font_bytes::PLEX_THAI)?,
            noto()?,
        ],
        strong: [
            f(font_bytes::PLEX_SANS_SB)?,
            f(font_bytes::PLEX_THAI_SB)?,
            noto()?,
        ],
        mono: [
            f(font_bytes::PLEX_MONO)?,
            f(font_bytes::PLEX_THAI)?,
            noto()?,
        ],
        mono_strong: [
            f(font_bytes::PLEX_MONO_SB)?,
            f(font_bytes::PLEX_THAI_SB)?,
            noto()?,
        ],
    })
}
/// Font chain and pixel size for a style at a paper width.
fn face(
    faces: &Faces,
    style: Style,
    money: bool,
    width: u32,
    scale: f32,
) -> (&[FontRef<'static>; 3], f32) {
    let k = if width >= 500 { 1.15 } else { 1.0 } * scale;
    let (chain, px) = match (style, money) {
        (Style::Small, false) => (&faces.sans, 19.0),
        (Style::Small, true) => (&faces.mono, 19.0),
        (Style::Body, false) => (&faces.sans, 23.0),
        (Style::Body, true) => (&faces.mono, 23.0),
        (Style::Strong, false) => (&faces.strong, 23.0),
        (Style::Strong, true) => (&faces.mono_strong, 23.0),
        (Style::Title, _) => (&faces.strong, 30.0),
        (Style::Total, false) => (&faces.strong, 30.0),
        (Style::Total, true) => (&faces.mono_strong, 34.0),
    };
    (chain, px * k)
}
/// The first font in the chain that has a glyph for `c`.
fn pick<'a>(chain: &'a [FontRef<'static>; 3], c: char) -> &'a FontRef<'static> {
    chain
        .iter()
        .find(|f| f.glyph_id(c).0 != 0)
        .unwrap_or(&chain[0])
}
fn measure(chain: &[FontRef<'static>; 3], px: f32, text: &str) -> f32 {
    text.chars()
        .map(|c| {
            let f = pick(chain, c);
            f.as_scaled(PxScale::from(px)).h_advance(f.glyph_id(c))
        })
        .sum()
}
/// Height of one line, with room for Thai marks above and below.
fn line_height(chain: &[FontRef<'static>; 3], px: f32) -> (f32, f32) {
    let scale = PxScale::from(px);
    let ascent = chain
        .iter()
        .map(|f| f.as_scaled(scale).ascent())
        .fold(0.0, f32::max);
    let descent = chain
        .iter()
        .map(|f| f.as_scaled(scale).descent())
        .fold(0.0, f32::min);
    (ascent, ascent - descent + px * 0.08)
}
fn draw(
    img: &mut GrayImage,
    chain: &[FontRef<'static>; 3],
    px: f32,
    text: &str,
    x: f32,
    baseline: f32,
) {
    let scale = PxScale::from(px);
    let mut pen = x;
    for c in text.chars() {
        let f = pick(chain, c);
        let id = f.glyph_id(c);
        let glyph = id.with_scale_and_position(scale, point(pen, baseline));
        if let Some(outline) = f.outline_glyph(glyph) {
            let b = outline.px_bounds();
            outline.draw(|gx, gy, coverage| {
                let px = b.min.x as i64 + i64::from(gx);
                let py = b.min.y as i64 + i64::from(gy);
                if let (Ok(px), Ok(py)) = (u32::try_from(px), u32::try_from(py))
                    && px < img.width()
                    && py < img.height()
                {
                    let ink = (coverage.clamp(0.0, 1.0) * 255.0) as u8;
                    let old = img.get_pixel(px, py).0[0];
                    img.put_pixel(px, py, Luma([old.saturating_sub(ink)]));
                }
            });
        }
        pen += f.as_scaled(scale).h_advance(id);
    }
}

enum Op {
    Text {
        style: Style,
        money: bool,
        text: String,
        x: f32,
        baseline: f32,
    },
    Rule(f32),
    Bitmap(Bitmap, u32, u32),
}
/// A QR code about 45% of the paper width with a quiet zone.
fn qr_bitmap(payload: &str, width: u32) -> Option<Bitmap> {
    let code = qrcodegen::QrCode::encode_text(payload, qrcodegen::QrCodeEcc::Medium).ok()?;
    let modules = u32::try_from(code.size()).ok()? + 8;
    let cell = (width * 45 / 100 / modules).max(2);
    let side = modules * cell;
    let mut ink = vec![false; (side * side) as usize];
    for y in 0..side {
        for x in 0..side {
            let mx = i32::try_from(x / cell).ok()? - 4;
            let my = i32::try_from(y / cell).ok()? - 4;
            if code.get_module(mx, my) {
                ink[(y * side + x) as usize] = true;
            }
        }
    }
    Some(Bitmap {
        width: side,
        height: side,
        ink,
    })
}
/// Renders `doc` at `width` dots. Returns None only if the bundled fonts fail to load.
pub fn render(doc: &Doc, width: u32) -> Option<GrayImage> {
    let faces = faces()?;
    let scale = doc.size.scale();
    let margin = 6.0;
    let inner = width as f32 - margin * 2.0;
    let mut ops = Vec::new();
    let mut y = margin;
    for block in &doc.blocks {
        match block {
            Block::Image(b) => {
                let w = b.width.min(width);
                let x = (width - w) / 2;
                ops.push(Op::Bitmap(b.clone(), x, y as u32));
                y += b.height as f32 + 8.0;
            }
            Block::Qr(payload) => {
                if let Some(b) = qr_bitmap(payload, width) {
                    let x = (width.saturating_sub(b.width)) / 2;
                    let h = b.height;
                    ops.push(Op::Bitmap(b, x, y as u32));
                    y += h as f32 + 8.0;
                }
            }
            Block::Text { text, style, align } => {
                let (chain, px) = face(&faces, *style, false, width, scale);
                let (ascent, lh) = line_height(chain, px);
                let (broken, _) = break_lines(&|s| measure(chain, px, s), text, inner, 0);
                for line in broken.lines() {
                    let w = measure(chain, px, line);
                    let x = match align {
                        Align::Left => margin,
                        Align::Center => margin + ((inner - w) / 2.0).max(0.0),
                    };
                    ops.push(Op::Text {
                        style: *style,
                        money: false,
                        text: line.to_owned(),
                        x,
                        baseline: y + ascent,
                    });
                    y += lh;
                }
            }
            Block::Row {
                left,
                right,
                style,
                indent,
            } => {
                let (chain, px) = face(&faces, *style, false, width, scale);
                let (mchain, mpx) = face(&faces, *style, true, width, scale);
                let (ascent, lh) = line_height(chain, px);
                let right_w = measure(mchain, mpx, right);
                let x0 = margin + if *indent { px * 1.4 } else { 0.0 };
                let gap = if right.is_empty() { 0.0 } else { px * 0.6 };
                let room = (margin + inner - right_w - gap - x0).max(px * 4.0);
                let (broken, _) = break_lines(&|s| measure(chain, px, s), left, room, 0);
                let first = y;
                for line in broken.lines() {
                    ops.push(Op::Text {
                        style: *style,
                        money: false,
                        text: line.to_owned(),
                        x: x0,
                        baseline: y + ascent,
                    });
                    y += lh;
                }
                if broken.is_empty() {
                    y += lh;
                }
                if !right.is_empty() {
                    let (m_ascent, _) = line_height(mchain, mpx);
                    ops.push(Op::Text {
                        style: *style,
                        money: true,
                        text: right.clone(),
                        x: margin + inner - right_w,
                        baseline: first + ascent.max(m_ascent),
                    });
                }
            }
            Block::Divider => {
                y += 6.0;
                ops.push(Op::Rule(y));
                y += 8.0;
            }
            Block::Space => y += 14.0,
        }
    }
    let height = (y + margin).ceil().max(1.0) as u32;
    let mut img = GrayImage::from_pixel(width, height, Luma([255]));
    for op in ops {
        match op {
            Op::Text {
                style,
                money,
                text,
                x,
                baseline,
            } => {
                let (chain, px) = face(&faces, style, money, width, scale);
                draw(&mut img, chain, px, &text, x, baseline);
            }
            Op::Bitmap(b, x0, y0) => {
                for by in 0..b.height {
                    for bx in 0..b.width {
                        let (px, py) = (x0 + bx, y0 + by);
                        if b.ink[(by * b.width + bx) as usize] && px < width && py < height {
                            img.put_pixel(px, py, Luma([0]));
                        }
                    }
                }
            }
            Op::Rule(y) => {
                // Dashed rule, two dots thick.
                let row = y.round() as u32;
                for x in (margin as u32)..(width - margin as u32) {
                    if (x / 6) % 2 == 0 {
                        for dy in 0..2 {
                            if row + dy < height {
                                img.put_pixel(x, row + dy, Luma([0]));
                            }
                        }
                    }
                }
            }
        }
    }
    Some(img)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thai_receipts_render_at_paper_width() {
        let mut doc = Doc::default();
        doc.text("ร้านหัวใจ", Style::Title, Align::Center);
        doc.row(
            "2 × ชาไทยเย็น / Thai iced tea หวานน้อย ไข่มุก",
            "110.00",
            Style::Body,
        );
        doc.detail("ส่วนลด 10%", "−11.00");
        doc.divider();
        doc.row("ยอดรวม", "฿99.00", Style::Total);
        for width in [384, 576] {
            let img = render(&doc, width);
            assert!(img.is_some());
            if let Some(img) = img {
                assert_eq!(img.width(), width);
                // Something was drawn, and the long name wrapped.
                assert!(img.pixels().any(|p| p.0[0] < 64));
                assert!(img.height() > 150);
            }
        }
    }
    #[test]
    fn logos_and_qr_codes_print_centred_and_text_scales() {
        let logo = Bitmap {
            width: 100,
            height: 40,
            ink: vec![true; 4_000],
        };
        let mut doc = Doc::default();
        doc.image(logo);
        doc.qr("00020101021129370016A000000677010111011300668123456785802TH53037646304ABCD");
        let img = render(&doc, 384);
        assert!(img.is_some());
        if let Some(img) = img {
            // The logo starts at (384 - 100) / 2.
            assert_eq!(img.get_pixel(142, 7).0[0], 0);
            assert_eq!(img.get_pixel(141, 7).0[0], 255);
            assert!(img.height() > 40 + 150);
        }
        let mut small = Doc::default();
        small.text("ร้านหัวใจ", Style::Title, Align::Center);
        let mut large = small.clone();
        large.size = TextSize::Large;
        let h = |d: &Doc| render(d, 384).map_or(0, |i| i.height());
        assert!(h(&large) > h(&small));
    }
}
