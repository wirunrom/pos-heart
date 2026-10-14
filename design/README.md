# POS Design System v0.2 — ชุดสำหรับนำไปใช้ในโค้ด

แตกโฟลเดอร์นี้ไว้ใน repo ที่ `design/` (หรือ `docs/design-system/`) แล้ว commit เป็นส่วนหนึ่งของโปรเจกต์
ต้นฉบับที่แก้ไขได้ยังอยู่ใน Design System artifact — เมื่อแก้ที่นั่นให้ export ชุดนี้ใหม่แล้วแทนที่ทั้งโฟลเดอร์

## ข้างในมีอะไร

| โฟลเดอร์ | ใช้ทำอะไร | นำไปใช้ยังไง |
|---|---|---|
| `rust/tokens.rs` | สี (ธีมเดียว + ชุดสีหลัก 5 ชุด), ระยะ, radius, ขนาดตาม density (compact/touch), type scale, motion ในรูปแบบ Rust/egui | **copy เข้า crate UI** เช่น `src/ui/tokens.rs` · compile ผ่านแล้ว (ต้องใช้ `egui::Color32`) |
| `tokens/tokens.dtcg.json` | ต้นฉบับ token 3 ชั้น (W3C DTCG) + modes | ใช้สร้าง `tokens.rs` / CSS ใหม่เมื่อ token เปลี่ยน · ใช้กับ web back office ในอนาคต |
| `tokens/tokens.json` | รูปแบบที่ Design System artifact อ่าน | เก็บไว้อ้างอิง |
| `docs/` | foundations, tokens, components, patterns, IA, governance, **07-egui-handoff** | อ่าน 07 ก่อนเริ่มเขียน widget · `docs/components/*.md` = สเปกแต่ละ component |
| `reference/` | `pos-desktop-concept.html` (เปิดใน browser ได้เลย กดได้ทุกหน้า) + ภาพหน้าจอ compact/touch | ใช้เทียบหน้าตาและพฤติกรรม — **ไม่ใช่โค้ดของแอป** |

## ลำดับที่แนะนำ

1. **ทดสอบฟอนต์ไทยใน egui ก่อน** (docs/07 ข้อ 2) — ฝัง IBM Plex Sans Thai, IBM Plex Sans, IBM Plex Mono (ดาวน์โหลดจาก Google Fonts / GitHub IBM/plex, สัญญาอนุญาต OFL) แล้ว render `ปั้น กี่ ที่นี่ ญาติ ฐาน ฎีกา น้ำ` ที่ 12–22px
2. **ใส่ `tokens.rs` + เขียน `Theme::apply(ctx)` และโครงหน้าต่าง** (title bar · toolbar · panes · status bar) ตาม docs/07 ข้อ 1 · สลับ Compact/Touch ได้
3. **ทำ widget ชุดแรก:** Button, MoneyField, CartLine + LineEditor, ProductTile, OrderSummary (ชำระ 2 ขั้น) — ใช้ `Palette`/`Metrics` เท่านั้น
4. **ประกอบหน้าขาย** เทียบกับ `reference/pos-desktop-concept.html` ที่ 1280×820 และ 1000×700
5. ทำหน้าที่เหลือโดยใช้ widget เดิม

```rust
use crate::ui::tokens::{Palette, Metrics, Density, AccentPreset, space, radius, type_scale};

let p = Palette::DEFAULT; // หรือ .with_accent(AccentPreset::Jade)
let m = Metrics::for_density(Density::Touch);
let w = m.cart_width;                              // 392 compact / 430 touch
let pay_bg = p.button_primary_bg();                // accent (saffron) — 1 ปุ่มต่อมุมมอง
```

## ใส่ใน CLAUDE.md ของโปรเจกต์ (แนะนำ)

ถ้าใช้ Claude Code ใน repo นี้ เพิ่มส่วนนี้ใน `CLAUDE.md` เพื่อให้ทุกงาน UI ยึดระบบเดียวกัน:

```markdown
## Design system
- UI follows `design/` (POS Design System v0.1). Read `design/docs/07-egui-handoff.md` before UI work,
  and the matching `design/docs/components/<Name>.md` before building or changing a widget.
- Colours/sizes come only from `src/ui/tokens.rs` (`Palette`, `Metrics`, `space`, `radius`, `type_scale`). Never hard-code `Color32` or pixel sizes in widgets.
- Money: store as i64 satang; display with 2 decimals, `IBM Plex Mono`, right-aligned, never truncated.
- One accent (primary) button per view. Key actions always have a text label and a keyboard shortcut.
- Single theme (graphite chrome + paper panes). Check every UI change in ไทย + English, compact + touch, at 1000×700 and 1280×820.
- Line actions live in the line editor; bill actions in the toolbar group “ทั้งบิล”. Disabled primary buttons are grey and state the reason.
- `design/reference/` is a visual reference only — do not port its HTML/CSS/JS.
```

## เปลี่ยนสีหลัก / แบรนด์

แก้ใน Design System artifact (หรือ `tokens.dtcg.json` → `brand.accent-ramp`) แล้วสร้าง `tokens.rs` ใหม่ · ไม่ต้องแก้ widget · ชุดสีที่ทดสอบแล้วดูใน docs/02 “สีหลักที่เตรียมไว้”
