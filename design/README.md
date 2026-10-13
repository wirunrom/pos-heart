# POS Design System v0.1 — ชุดสำหรับนำไปใช้ในโค้ด

แตกโฟลเดอร์นี้ไว้ใน repo ที่ `design/` (หรือ `docs/design-system/`) แล้ว commit เป็นส่วนหนึ่งของโปรเจกต์
ต้นฉบับที่แก้ไขได้ยังอยู่ใน Design System artifact — เมื่อแก้ที่นั่นให้ export ชุดนี้ใหม่แล้วแทนที่ทั้งโฟลเดอร์

## ข้างในมีอะไร

| โฟลเดอร์ | ใช้ทำอะไร | นำไปใช้ยังไง |
|---|---|---|
| `rust/tokens.rs` | สี (Light/Dark), ระยะ, radius, ขนาดตาม density, type scale, motion ในรูปแบบ Rust/egui | **copy เข้า crate UI** เช่น `src/ui/tokens.rs` · compile ผ่านแล้ว (ต้องใช้ `egui::Color32`) |
| `tokens/tokens.dtcg.json` | ต้นฉบับ token 3 ชั้น (W3C DTCG) + modes | ใช้สร้าง `tokens.rs` / CSS ใหม่เมื่อ token เปลี่ยน · ใช้กับ web back office ในอนาคต |
| `tokens/tokens.json` | รูปแบบที่ Design System artifact อ่าน | เก็บไว้อ้างอิง |
| `docs/` | foundations, tokens, components, patterns, IA, governance, **07-egui-handoff** | อ่าน 07 ก่อนเริ่มเขียน widget · `docs/components/*.md` = สเปกแต่ละ component |
| `reference/` | `pos-sell-prototype.html` (เปิดใน browser ได้เลย), ภาพหน้าจอ, CSS/JS อ้างอิง | ใช้เทียบหน้าตาและพฤติกรรม — **ไม่ใช่โค้ดของแอป** |

## ลำดับที่แนะนำ

1. **ทดสอบฟอนต์ไทยใน egui ก่อน** (docs/07 ข้อ 2) — ฝัง IBM Plex Sans Thai, IBM Plex Sans, IBM Plex Mono (ดาวน์โหลดจาก Google Fonts / GitHub IBM/plex, สัญญาอนุญาต OFL) แล้ว render `ปั้น กี่ ที่นี่ ญาติ ฐาน ฎีกา น้ำ` ที่ 12–22px
2. **ใส่ `tokens.rs` + เขียน `Theme::apply(ctx)`** ตาม docs/07 ข้อ 1 · สลับ Light/Dark/System และ Compact/Touch ได้
3. **ทำ widget ชุดแรก:** Button, MoneyField, CartLine, ProductTile — ใช้ `Palette`/`Metrics` เท่านั้น ห้ามใส่ค่าสีตรง ๆ
4. **ประกอบหน้าขายที่ 1000×700** เทียบกับ `reference/pos-sell-prototype.html`
5. ทำหน้าที่เหลือโดยใช้ widget เดิม

```rust
use crate::ui::tokens::{Palette, Metrics, ThemeMode, Density, space, radius, type_scale};

let p = Palette::for_mode(ThemeMode::Dark);
let m = Metrics::for_density(Density::Touch);
let w = m.cart_width(ctx.screen_rect().width());   // 360 / 392 / 420
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
- Check every UI change in Light + Dark, ไทย + English, compact + touch, at 1000×700 and 1280×820.
- `design/reference/` is a visual reference only — do not port its HTML/CSS/JS.
```

## เปลี่ยนสีหลัก / แบรนด์

แก้ใน Design System artifact (หรือ `tokens.dtcg.json` → `brand.accent-ramp`) แล้วสร้าง `tokens.rs` ใหม่ · ไม่ต้องแก้ widget · ชุดสีที่ทดสอบแล้วดูใน docs/02 “สีหลักที่เตรียมไว้”
