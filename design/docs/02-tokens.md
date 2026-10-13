# 02 Tokens

## โครงสร้าง 3 ชั้น (และไม่มากกว่านั้น)

```
brand.*            ชื่อผลิตภัณฑ์, logo, ramp ที่ใช้เป็น accent      ← เปลี่ยนแบรนด์ที่นี่ที่เดียว
   │
primitive.*        stone-50 … stone-975, saffron-100 … 950, green/orange/red/blue   ← ค่าดิบ ไม่มีความหมาย
   │
semantic.*         bg-surface, fg-muted, accent-bg, danger-fg, space-3, radius-lg, control-md …
   │               ← component ใช้ได้แค่ชั้นนี้ (+ ชั้นล่าง)
component.*        button-primary-bg, tile-selected-border, cartline-selected-bg …
                   ← สร้างเฉพาะเมื่อ component ต้องปรับแยกจาก semantic
```

**กติกา:** component **ห้าม**อ้าง primitive ตรง ๆ · ทุก component token ต้องชี้ไปที่ semantic token · จะเพิ่ม component token ได้ต่อเมื่อมีเหตุผล (เช่น ต้องการให้ธีมลูกค้าปรับปุ่มหลักได้โดยไม่กระทบส่วนอื่น) ไม่ใช่เพื่อให้ดูครบ

## การตั้งชื่อ

`{กลุ่ม}-{บทบาท}[-{ตัวแปร}][-{สถานะ}]` ใช้ตัวพิมพ์เล็กและขีดกลาง

| กลุ่ม | ตัวอย่าง | หมายเหตุ |
|---|---|---|
| `bg-` | `bg-canvas` `bg-surface` `bg-raised` `bg-sunken` `bg-hover` `bg-pressed` `bg-selected` `bg-inverse` | พื้น |
| `fg-` | `fg-default` `fg-muted` `fg-subtle` `fg-inverse` `fg-accent` `fg-on-accent` `fg-on-danger` | ข้อความ/icon · `on-*` = บนพื้นสีทึบ |
| `border-` | `border-subtle` `border-default` `border-control` `border-selected` · `focus-ring` | |
| `accent-` | `accent-bg` `accent-bg-hover` `accent-bg-pressed` `accent-soft` | มาจาก `brand.accent-ramp` |
| `{status}-` | `success-fg` `success-bg` `success-solid` (ใช้แบบเดียวกันกับ warning/danger/info) | |
| `chart-` | `chart-1…4` `chart-grid` | |
| ขนาด | `space-*` `radius-*` `control-*` `row-height` `icon-*` `layer-*` `duration-*` | |

**ห้าม:** ตั้งชื่อตามสี (`yellow-button`), ตามหน้าจอ (`sell-cart-bg`), หรือมีชื่อผลิตภัณฑ์ (`heart-primary`)

## ตาราง mapping หลัก (Light / Dark)

| Semantic | Light | Dark | ใช้กับ |
|---|---|---|---|
| `bg-canvas` | stone-50 `#f5f4f0` | stone-950 `#121110` | พื้นแอป |
| `bg-surface` | stone-0 `#ffffff` | stone-925 `#191816` | ตะกร้า, ตาราง |
| `bg-raised` | stone-0 | stone-900 `#1f1e1b` | การ์ดสินค้า, เมนู |
| `bg-sunken` | stone-100 `#ebe9e3` | stone-975 `#0b0b0a` | สรุปยอด |
| `bg-selected` | saffron-100 `#fcf0d4` | saffron-950 `#2a1e09` | บรรทัดที่เลือก |
| `fg-default` | stone-900 `#1f1e1b` | stone-50 | ข้อความ, เงิน |
| `fg-muted` | stone-600 `#625e55` | stone-400 `#a8a398` | ป้าย, metadata |
| `border-control` | stone-500 `#827d72` | stone-500 | ขอบ input |
| `accent-bg` | saffron-400 `#f0b232` | saffron-400 | ปุ่ม commit |
| `fg-on-accent` | stone-950 | stone-950 | ข้อความบน accent (เข้มทั้งสองธีม) |
| `focus-ring` | blue-600 `#2563c7` | blue-300 `#93b9f3` | focus |
| `danger-fg` | red-700 `#9a2a21` | red-300 `#f59a90` | ข้อผิดพลาด, เงินคืน |

รายการครบทุกตัวอยู่ในแท็บ Colors และ `tokens.json`

## สถานะการโต้ตอบ (interaction states)

| สถานะ | ปุ่มกลาง (neutral) | ปุ่มหลัก (primary) | แถว / การ์ด |
|---|---|---|---|
| Rest | `bg-surface` + `border-default` | `accent-bg` | `bg-surface` / `bg-raised` |
| Hover (เมาส์เท่านั้น) | `bg-hover` | `accent-bg-hover` | `bg-hover` / `border-control` |
| Pressed | `bg-pressed` | `accent-bg-pressed` | scale 0.985 (การ์ด) |
| Focus | + `focus-ring` 2px offset 2 | + `focus-ring` | + `focus-ring` |
| Selected | — | — | `bg-selected` หรือ `border-selected` 2px |
| Disabled | `opacity-disabled` 0.45 + ไม่รับการกด + บอกเหตุผลถ้าทำได้ | เหมือนกัน | `bg-sunken` + ข้อความบอกสาเหตุ ("หมด", "ปิดขาย") |
| Loading | เก็บความกว้างไว้, แสดง spinner, ไม่รับการกดซ้ำ | เหมือนกัน | skeleton |

## Density และ platform

`tokens.json` (ไฟล์ที่หน้านี้อ่าน) เก็บสีตามธีม + ค่า **compact** · ค่า **touch** และ platform อยู่ใน `tokens/tokens.dtcg.json → modes` และใน `components/bundle.css` (`[data-density="touch"]`)

```json
"density": { "touch": { "overrides": {
  "size.control-md": "48px", "size.row-height": "56px",
  "font.style.body": "16px/26px", "font.style.money": "16px/24px" } } },
"platform": { "kds": { "density": "touch", "theme": "dark", "font.scale": 1.25 } }
```

## ไฟล์

| ไฟล์ | รูปแบบ | ใช้ทำอะไร |
|---|---|---|
| `tokens.json` | รายการแบน (list) ที่หน้านี้อ่านได้ | แสดงผลในระบบนี้, สร้าง CSS |
| `tokens/tokens.dtcg.json` | W3C Design Tokens (DTCG) แบ่ง 3 ชั้น + modes | ต้นฉบับสำหรับ generator: Rust (egui), CSS (web back office), Swift/Kotlin ในอนาคต |
| `tokens.css` | สร้างอัตโนมัติ | CSS custom properties |

**Pipeline ที่แนะนำ:** `tokens.dtcg.json` → script (เช่น Style Dictionary หรือ `build.rs` เล็ก ๆ) → `theme_tokens.rs` (ค่า const + `fn semantic(theme) -> Palette`) และ `tokens.css` ให้ทุกแพลตฟอร์มใช้ต้นฉบับเดียวกัน

## เปลี่ยนแบรนด์ (rebrand)

1. แก้ `brand.name` และใส่ logo ในกลุ่ม asset `Logos`
2. ถ้าต้องการเปลี่ยนสี ให้เพิ่ม primitive ramp ใหม่ (เช่น `jade-100…950`) แล้วเปลี่ยน `brand.accent-ramp`
3. ตรวจ `fg-on-accent` บน `accent-bg` / `-hover` / `-pressed` ให้ได้ ≥ 4.5:1 (ramp ที่สว่างใช้ข้อความเข้ม ramp ที่เข้มใช้ข้อความขาว)
4. ไม่ต้องแก้ component แม้แต่ตัวเดียว

## สีหลักที่เตรียมไว้ (accent presets) · Experimental

ลองสลับได้ใน prototype (“สีหลัก”) · ทุกชุดผ่าน contrast ทั้ง Light และ Dark (ข้อความบนปุ่ม ≥ 4.5:1 ทุกสถานะ) · เปลี่ยนแค่ `accent-*`, `fg-accent`, `fg-on-accent`, `bg-selected` ส่วนอื่นไม่ต้องแก้

| ชุด | `accent-bg` Light / Dark | ข้อความบนปุ่ม | หมายเหตุ |
|---|---|---|---|
| Saffron (ค่าเริ่มต้น) | `#f0b232` / `#f0b232` | เข้ม | อบอุ่น โดดเด่น · ต่างจากพื้นขาวน้อย ปุ่มจึงต้องมีข้อความเสมอ |
| Jade | `#0f7a5f` / `#2fae8a` | ขาว / เข้ม | สงบ ดูเป็นการเงิน · ใกล้สี success ให้ใช้ข้อความกำกับ |
| Ocean | `#1d6a8c` / `#4aa6cf` | ขาว / เข้ม | เป็นทางการ · ใกล้สี focus/info ควรตรวจว่า focus ring ยังเด่น |
| Plum | `#7b3c8f` / `#c07ad6` | ขาว / เข้ม | มีเอกลักษณ์ ไม่ชนกับสีสถานะใด |
| Ink | `#1f1e1b` / `#f5f4f0` | ขาว / เข้ม | เรียบที่สุด ไม่มีสีแบรนด์ · ปุ่มหลักเด่นด้วยน้ำหนักแทนสี |

ถ้าจะใช้ชุดใดถาวร: เพิ่ม primitive ramp ของชุดนั้นใน `tokens.dtcg.json` แล้วเปลี่ยน `brand.accent-ramp` (ดูหัวข้อ “เปลี่ยนแบรนด์” ด้านบน)
