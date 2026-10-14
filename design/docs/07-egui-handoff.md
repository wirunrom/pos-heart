# 07 egui handoff

ระบบนี้ไม่ผูกกับ Rust: ต้นฉบับคือ `tokens/tokens.dtcg.json` + สเปกในเอกสาร ส่วน HTML ในการ์ดเป็นแค่ reference
หน้านี้บอกวิธีทำให้ได้จริงใน **egui + eframe** โดยไม่เปลี่ยน stack · ชื่อ API อ้างอิง egui 0.29–0.32 · v0.2 ใช้ธีมเดียว ไม่ต้องรองรับ Dark (บางชื่อเปลี่ยนตามเวอร์ชัน เช่น `rounding` → `corner_radius`, `Frame::none()` → `Frame::new()`) **ต้องตรวจกับเวอร์ชันที่ใช้จริง**

## 1. โครงสร้าง theme / token (v0.2 ธีมเดียว)

```
crates/ui/src/tokens.rs   ← สร้างจาก tokens.dtcg.json (อยู่ใน zip) — Palette::DEFAULT, Metrics::{COMPACT,TOUCH}, radius, space, type_scale, motion
crates/ui/src/theme.rs    ← Theme { palette, metrics, shape, accent }, apply(ctx)
crates/ui/src/chrome/*.rs ← title_bar.rs, toolbar.rs, status_bar.rs, pane.rs, save_bar.rs
crates/ui/src/widgets/*   ← button, money_field, cart_line, product_tile …
```

```rust
pub struct Theme { pub p: Palette, pub m: Metrics, pub soft: bool, pub accent: AccentPreset }
impl Theme {
    pub fn apply(&self, ctx: &egui::Context) {
        let p = &self.p;
        let mut v = egui::Visuals::light();
        v.panel_fill = p.bg_canvas;                 // พื้นใต้แผง = กราไฟต์
        v.window_fill = p.bg_raised; v.extreme_bg_color = p.bg_raised; v.faint_bg_color = p.bg_sunken;
        v.selection.bg_fill = p.bg_selected; v.selection.stroke = Stroke::new(1.0, p.border_selected);
        let r = if self.soft { radius::MD } else { 3 };
        for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active] { w.corner_radius = r.into(); }
        v.widgets.inactive.weak_bg_fill = p.bg_raised; v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.border_default);
        v.widgets.hovered.weak_bg_fill = p.bg_sunken; v.widgets.active.weak_bg_fill = p.bg_pressed;
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.fg_default);
        ctx.set_visuals(v);
        ctx.style_mut(|s| { s.spacing.interact_size.y = self.m.control_md; s.spacing.item_spacing = egui::vec2(8.0, 8.0); });
    }
    pub fn pane_frame(&self) -> egui::Frame {
        egui::Frame::new().fill(self.p.bg_surface).corner_radius(if self.soft { radius::LG } else { 0 })
    }
}
```

**โครงหน้าต่าง:**

```rust
TopBottomPanel::top("titlebar").exact_height(m.titlebar_h).frame(Frame::new().fill(p.bg_chrome)).show(ctx, |ui| title_bar(ui, &t, &mut st));
TopBottomPanel::top("toolbar").exact_height(m.toolbar_h).frame(Frame::new().fill(p.bg_chrome_raised)).show(ctx, |ui| toolbar(ui, &t, &mut st));
TopBottomPanel::bottom("status").exact_height(m.statusbar_h).frame(Frame::new().fill(p.bg_chrome)).show(ctx, |ui| status_bar(ui, &t, &mut st));
CentralPanel::default().frame(Frame::new().fill(p.bg_canvas).inner_margin(Margin { left: 6, right: 6, top: 0, bottom: 6 })).show(ctx, |ui| {
    // แต่ละแผง = ui.allocate_ui_at_rect(rect, |ui| t.pane_frame().show(ui, ...)) โดยแบ่ง rect เอง เว้น space::PANE_GAP
});
```

**Unified title bar:** macOS — `ViewportBuilder::default().with_fullsize_content_view(true).with_titlebar_shown(false).with_title_shown(false)` แล้วเว้นที่ปุ่มหน้าต่างซ้าย ~70px; ลากหน้าต่างด้วย `ctx.send_viewport_cmd(ViewportCommand::StartDrag)` เมื่อกดบนพื้นที่ว่างของ title bar · Windows/Linux — `with_decorations(false)` และวาดปุ่มย่อ/ขยาย/ปิดเองทางขวา (`ViewportCommand::Minimized/Maximized/Close`) · **ทางสำรอง** ถ้ายังไม่พร้อม: ใช้ title bar ของ OS แล้ววางแท็บเป็นแถวแรกของแอป (เสียพื้นที่ ~30px)

**เสียง feedback:** ไม่มีใน egui — ใช้ crate เล็ก เช่น `rodio` เล่นไฟล์ wav สั้น 2 ไฟล์ (สำเร็จ / ผิดพลาด) บน thread แยก · ปิดได้จากค่าที่เก็บใน `eframe::Storage`

## 2. ฟอนต์

```rust
let mut f = egui::FontDefinitions::default();
f.font_data.insert("ui_thai".into(), Arc::new(FontData::from_static(include_bytes!("../fonts/IBMPlexSansThai-Regular.ttf"))));
f.font_data.insert("ui_latin".into(), Arc::new(FontData::from_static(include_bytes!("../fonts/IBMPlexSans-Regular.ttf"))));
f.font_data.insert("num".into(), Arc::new(FontData::from_static(include_bytes!("../fonts/IBMPlexMono-Medium.ttf"))));
let prop = f.families.entry(FontFamily::Proportional).or_default();
prop.insert(0, "ui_latin".into()); prop.insert(1, "ui_thai".into());           // fallback ตามลำดับ
f.families.insert(FontFamily::Name("num".into()), vec!["num".into(), "ui_thai".into()]);
ctx.set_fonts(f);
```

- egui เลือกน้ำหนักฟอนต์ไม่ได้ในตัว → ลงทะเบียน Medium/SemiBold เป็น family แยก (`FontFamily::Name("ui_semibold")`)
- egui **ไม่มี OpenType features** → เงินต้องใช้ family `num` (Plex Mono) เท่านั้น จึงตรงหลักโดยไม่ต้องใช้ `tnum`
- ⚠️ **ความเสี่ยงอันดับ 1: การวางสระ/วรรณยุกต์ไทย** — text layout ของ egui ไม่ได้ทำ complex shaping (GPOS mark positioning) แบบ browser วรรณยุกต์บนสระบน (เช่น `ปั้น`, `ที่นี่`, `กี่`) อาจซ้อนกันหรือลอยผิดตำแหน่ง ขึ้นกับฟอนต์และเวอร์ชัน egui
  **ทำก่อนเริ่ม:** render สตริงทดสอบ `ปั้น กี่ ที่นี่ ญาติ ฐาน ฎีกา ฤๅษี น้ำ` ที่ 12/14/16/22px ด้วย Plex Sans Thai, Noto Sans Thai และ Sarabun แล้วเลือกตัวที่ผิดน้อยสุด
  **ทางเลือกถ้ายังไม่ผ่าน (ไม่เปลี่ยน stack):** (ก) เลือกฟอนต์ที่ตำแหน่ง mark ปกติดีพอ (b) ใช้ text renderer ที่รองรับ shaping เช่น cosmic-text/swash เฉพาะ label ที่เป็นไทย ผ่าน custom painter (c) ติดตามการปรับปรุง text layout ใน release ของ egui
- ⚠️ **การตัดบรรทัดภาษาไทย:** egui ตัดบรรทัดที่ช่องว่าง ภาษาไทยที่ไม่มีช่องว่างจะถูกตัดกลางคำ → ใช้ `icu_segmenter` (WordSegmenter) หาจุดตัด แล้วจัดบรรทัดเอง (`LayoutJob` แยกตามคำ) หรือแทรก break opportunity — ทดสอบว่าเวอร์ชันที่ใช้ตัดที่ U+200B ได้หรือไม่

## 3. Layout และการเลื่อน (scrolling)

```rust
TopBottomPanel::top("topbar").exact_height(m.topbar).show(ctx, |ui| topbar(ui, &theme, &state));
SidePanel::left("cart").exact_width(cart_w).resizable(false).frame(surface_frame(&theme)).show(ctx, |ui| {
    TopBottomPanel::bottom("cart_footer").show_inside(ui, |ui| { order_summary(ui, ..); quick_cash(ui, ..); pay_button(ui, ..); });
    CentralPanel::default().show_inside(ui, |ui| {
        ScrollArea::vertical().auto_shrink([false; 2]).stick_to_bottom(true).show(ui, |ui| for (i, l) in cart.lines.iter().enumerate() { cart_line(ui, l, i == cur) });
    });
});
CentralPanel::default().show(ctx, |ui| products_panel(ui, ..));
```

- `cart_w = if screen_w < 1100.0 { 360.0 } else if touch { 420.0 } else { 392.0 }`
- ส่วนท้ายตะกร้า (ยอด + ปุ่มรับเงิน) อยู่ใน bottom panel จึง**ไม่เลื่อนหายไป** · รายการเลื่อนได้และเลื่อนไปที่บรรทัดที่เพิ่งเพิ่ม (`scroll_to_me`)
- Grid การ์ดสินค้า: `cols = ((w + gap) / (min_w + gap)).floor().max(1.0)` แล้ววาดทีละแถวด้วย `ui.horizontal` + `allocate_exact_size` · ใช้ `ScrollArea::show_rows` ถ้าสินค้า > 500 รายการ
- ตาราง: `egui_extras::TableBuilder` (หัวตารางตรึงได้, คอลัมน์ตัวเลขชิดขวา, `.striped(false)`)
- ขอบจางที่ปลายรายการ: วาด gradient เองด้วย `painter.rect_filled` เป็นชั้นหลายชั้น หรือใช้ `Mesh` ที่ไล่สี (egui ไม่มี mask)

## 4. Component ที่ใช้ซ้ำได้: รูปแบบ API

Widget เป็น**ฟังก์ชันที่ไม่มีสถานะของตัวเอง**: รับ view-model คืน “intent” — app เป็นคนเปลี่ยนข้อมูล

```rust
pub enum CartLineIntent { Select, Inc, Dec, OpenOptions, Remove }
pub fn cart_line(ui: &mut Ui, t: &Theme, vm: &CartLineVm, current: bool) -> Option<CartLineIntent> { … }

pub fn money_field(ui: &mut Ui, t: &Theme, id: Id, value: &mut Satang, opts: MoneyFieldOpts) -> Response
// เก็บเงินเป็น i64 หน่วยสตางค์ (Satang) ห้ามใช้ f64 · ข้อความระหว่างแก้เก็บใน ui.data() ด้วย id

pub fn button(ui: &mut Ui, t: &Theme, label: &str) -> ButtonBuilder   // .primary() .danger() .size(Xl) .shortcut("F9") .loading(bool)
```

| ข้อมูล | เจ้าของ | ที่เก็บ |
|---|---|---|
| Domain (บิล, กะ, สินค้า) | App / service | SQLite ผ่าน repository · โหลดเข้า struct ใน state |
| สถานะของหน้าจอ (บรรทัดที่เลือก, คำค้น, ตัวกรอง, overlay ที่เปิด) | Screen state struct | ใน memory ของ app (บิลปัจจุบันเขียนลง DB ทุกครั้งที่เปลี่ยน) |
| สถานะชั่วคราวของ widget (ข้อความที่กำลังพิมพ์, popup เปิดอยู่, animation) | egui | `ui.data_mut` / `ui.memory` ด้วย `Id` |
| ค่าที่ผู้ใช้ตั้ง (theme, density, ภาษา) | App | `eframe::Storage` (ต่อเครื่อง) |

**คีย์บอร์ด:** จัดการที่ระดับหน้าจอ ไม่ใช่ใน widget — `ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F9))` · ถ้าไม่มี overlay เปิดอยู่และไม่มี TextEdit อื่นโฟกัส ให้ `search.request_focus()` ทุกเฟรม (รองรับเครื่องสแกน)

## 5. egui ทำได้ตรง ๆ vs ต้องทำเอง

| ทำได้ตรง ๆ (ปรับผ่าน Visuals/Style) | ต้องทำเอง (custom painter) | ข้อจำกัด / ทางเลือก |
|---|---|---|
| Panel, ScrollArea, Button พื้นฐาน, TextEdit, ComboBox (Select), Checkbox, Tooltip, Window/Modal, Grid, `TableBuilder`, Frame + shadow + corner radius, AccessKit, `pixels_per_point` (UI scale) | ProductTile, CartLine, OrderSummary (`ds-kv`), Segmented, Chip, Badge, StatusPill, Count, Steps, BulkBar, Sheet (เลื่อนเข้าด้วย `animate_value_with_time`), Skeleton, ขอบจางท้าย scroll, Toast stack, focus ring แบบมีระยะห่าง | Thai shaping + ตัดคำ (ข้อ 2) · ไม่มี OpenType features → ใช้ฟอนต์ mono สำหรับเงิน · ไม่มี CSS mask → ใช้ gradient mesh · กราฟ: `egui_plot` (เร็ว) หรือวาดเองเพื่อให้ตรงสเปก · ไม่มี hover บนจอสัมผัส → ออกแบบไม่ให้พึ่ง hover อยู่แล้ว |

**สิ่งที่แนะนำให้ทำก่อน (prototype ใน egui):** 1) ทดสอบฟอนต์ไทย 2) `Theme::apply` + โครงหน้าต่าง (title bar · toolbar · panes · status bar) + สลับ density 3) Button, MoneyField, CartLine + LineEditor, ProductTile, OrderSummary 4) หน้าขายเต็มหน้าที่ 1280×820 และ 1000×700 — ถ้า 4 ข้อนี้ผ่าน ระบบที่เหลือคือการทำซ้ำรูปแบบเดิม
