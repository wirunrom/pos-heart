# 02 Tokens

## 3 ชั้น

```
brand.*        ชื่อ, logo, accent-ramp (saffron | jade | ocean | plum | ink)
primitive.*    graphite-*, paper-*, slate-*, saffron-*, green/orange/red/blue-*, cat-*
semantic.*     bg-chrome, bg-surface, fg-muted, accent-bg, danger-fg, cat-food, space-3, radius-lg …   ← component ใช้ได้แค่ชั้นนี้
component.*    button-primary-bg, tile-selected-border, cartline-selected-bg, nav-item-active-bg
```

component ห้ามอ้าง primitive ตรง ๆ · component token สร้างเฉพาะเมื่อต้องปรับแยกจาก semantic

## การตั้งชื่อ

`{กลุ่ม}-{บทบาท}[-{ตัวแปร}]` ตัวเล็ก ขีดกลาง · กลุ่ม: `bg-` `fg-` `border-` `accent-` `{status}-` `cat-` `chart-` · ขนาด: `space-` `radius-` `control-` `row-height` `*-h` `*-width` · ห้ามมีชื่อสี ชื่อหน้าจอ หรือชื่อผลิตภัณฑ์

## ค่าหลัก (ธีมเดียว)

| Semantic | ค่า | ใช้กับ |
|---|---|---|
| `bg-chrome` | `#1f2023` | title bar, status bar |
| `bg-chrome-raised` / `bg-canvas` | `#2a2b2f` | toolbar, พื้นใต้แผง |
| `bg-chrome-hover` | `#35373b` | ปุ่ม chrome ตอน hover, แท็บที่เลือก |
| `fg-chrome` / `fg-chrome-muted` | `#ecebe7` / `#a6a7a2` | ข้อความบน chrome |
| `bg-surface` | `#f2f2ef` | แผง |
| `bg-raised` | `#ffffff` | การ์ด ตาราง ช่องกรอก |
| `bg-sunken` | `#e9e9e5` | track ของ segmented, ปุ่มที่กดไม่ได้, การ์ดที่ขายไม่ได้ |
| `bg-selected` + `border-selected` | `#dce3ed` + `#4f75aa` | สิ่งที่ถูกเลือก |
| `fg-default` / `fg-muted` / `fg-subtle` | `#1c1d1f` / `#5c5e5a` / `#868782` | ข้อความ |
| `border-subtle` / `default` / `control` | `#e0e0db` / `#cdcdc7` / `#868782` | เส้น |
| `focus-ring` | `#2f6fd0` | focus |
| `accent-bg` / `-hover` / `-pressed` / `fg-on-accent` | `#f0b232` / `#dc9810` / `#c48b10` / `#141310` | ปุ่มยืนยัน |
| `cat-food` `cat-drink` `cat-goods` `cat-snack` | `#c9781a` `#3f78b5` `#5f8f5a` `#a0569a` | สีหมวด |

ครบทุกตัวในแท็บ Colors / `tokens.json`

## สถานะการโต้ตอบ

| สถานะ | ปุ่มทั่วไป | ปุ่มหลัก | แถว / การ์ด | chrome |
|---|---|---|---|---|
| Hover | `bg-sunken` | `accent-bg-hover` | `bg-hover` | `bg-chrome-hover` |
| Pressed | `bg-pressed` | `accent-bg-pressed` | `bg-pressed` | — |
| Selected | — | — | `bg-selected` (+ `border-selected` บนการ์ด) | แท็บ: `bg-chrome-hover` + เส้น `accent-bg` |
| Focus | ring 2px `focus-ring` | เหมือนกัน | เหมือนกัน | เหมือนกัน |
| **Disabled** | ข้อความ `fg-subtle` | **พื้น `bg-sunken` + `fg-subtle` + ข้อความบอกเหตุผล** (ไม่ใช้สีหลักจาง) | `bg-sunken` + เหตุผล ("หมด") | opacity .4 |

## Density / shape

`tokens.json` เก็บค่า compact · ค่า touch และ square อยู่ใน `tokens/tokens.dtcg.json → modes` และ `components/bundle.css` (`[data-density="touch"]`)

## ชุดสีหลัก (brand presets)

ไม่ใช่ตัวเลือกของพนักงาน — ใช้ตอนกำหนดแบรนด์ · ทุกชุดผ่าน contrast · เปลี่ยนเฉพาะ `accent-*`, `fg-on-accent`, `accent-soft`, `chart-1`

| ชุด | accent | ข้อความบนปุ่ม | หมายเหตุ |
|---|---|---|---|
| **Saffron** (ค่าเริ่มต้น) | `#f0b232` | `#141310` | อบอุ่น เด่นบน chrome |
| Jade | `#0f7a5f` | ขาว | ใกล้สี success — ต้องมีข้อความ |
| Ocean | `#1d6a8c` | ขาว | ใกล้สี selection/focus |
| Plum | `#7b3c8f` | ขาว | ไม่ชนสีสถานะ |
| Ink | `#2c2d31` | ขาว | เรียบที่สุด |

## ไฟล์

| ไฟล์ | ใช้ทำอะไร |
|---|---|
| `tokens.json` | ที่ระบบนี้อ่าน (list) |
| `tokens/tokens.dtcg.json` | ต้นฉบับ 3 ชั้น + modes (density, shape, accent presets) สำหรับ generator |
| `tokens.rs` (ใน zip) | สร้างจาก dtcg สำหรับ egui |
