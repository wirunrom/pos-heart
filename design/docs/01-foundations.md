# 01 Foundations

ทุกค่ามี token (ดู 02) — ห้ามใส่สีหรือขนาดตรง ๆ ใน component

| เปลี่ยนตาม | สิ่งที่เปลี่ยน | คงที่ |
|---|---|---|
| **Density** compact / touch | ความสูง chrome/control/แถว, ขนาดตัวอักษร body/money, ไอคอน, คำใบ้คีย์ลัด, หมวดหมู่ (คอลัมน์ ↔ แถวปุ่ม) | สี, รูปทรง, layout |
| **Shape** soft / square | ระยะห่างแผง, ความโค้ง, รูปแบบไฮไลต์ | ทุกอย่างที่เหลือ |
| **Brand** | `brand.name`, logo, ชุดสีหลัก | ทุกอย่างที่เหลือ |
| **Platform** | density เริ่มต้น, ตัวคูณตัวอักษร (KDS 1.25, ลูกค้า 1.125), layout | tokens และ anatomy |

## สี: ธีมเดียว 2 ชั้น

| ชั้น | token | ใช้กับ |
|---|---|---|
| **Chrome** (กราไฟต์) | `bg-chrome` `bg-chrome-raised` `bg-chrome-hover` `fg-chrome` `fg-chrome-muted` `border-chrome` | title bar + แท็บ, toolbar, พื้นใต้แผง, status bar |
| **Workspace** (กระดาษ) | `bg-surface` (แผง) · `bg-raised` (การ์ด/ตาราง/ช่องกรอก ขาว) · `bg-sunken` | งานทั้งหมด |
| **Selection** | `bg-selected` (ฟ้าอมเทา) + `border-selected` | แถว/บรรทัด/หมวดที่เลือก · การ์ดที่อยู่ในบิล — แยกจากสีหลักโดยตั้งใจ |
| **Accent** (หญ้าฝรั่น) | `accent-bg` `fg-on-accent` `accent-soft` | ปุ่มยืนยันหนึ่งปุ่ม/มุมมอง, เส้นใต้แท็บที่เลือก, กะพริบ "เพิ่งเพิ่ม", กราฟชุดหลัก |
| **Status** | success / warning / danger / info (`-fg` `-bg` `-solid`) | ความหมายเท่านั้น และมีข้อความกำกับ |
| **Category** | `cat-food` `cat-drink` `cat-goods` `cat-snack` | แถบซ้ายการ์ดสินค้า, จุดในรายการหมวด — เป็น *ข้อมูล* ไม่ใช่สถานะ ร้านเลือกเพิ่มได้จากชุดที่ผ่าน contrast |

**ทำไมธีมเดียว:** หน้าตาคงที่ทุกเครื่อง · ไม่ต้องตรวจสองชุด · chrome เข้มลดแสงจ้ารอบขอบจอ ส่วนพื้นที่ทำงานสว่างพออ่านในร้านที่แสงจ้า
**Contrast (WCAG 2.2 AA) ตรวจด้วยสคริปต์ 31 คู่ ผ่านทั้งหมด:** ข้อความ ≥ 4.5:1 บนทุกพื้น (รวม chrome), ขอบ control/focus/selection/กราฟ ≥ 3:1 · `fg-subtle` ใช้กับ placeholder/disabled เท่านั้น

## ตัวอักษร

IBM Plex Sans Thai + IBM Plex Sans (`font-ui`) · **IBM Plex Mono สำหรับเงินทุกที่** (`font-num`) — ตัวเลขกว้างเท่ากันโดยไม่พึ่ง OpenType (egui ไม่รองรับ)
Scale (compact → touch): title 16 → 17 · body 13/20 → 15/22 · label 12 (toolbar 12.5) → 13–14 · caption 11.5 · money-xl 30 → 34 · money-lg 22 · money 13 → 16
ภาษาไทยต้อง line-height ≥ 1.45 · ไม่มีข้อความเล็กกว่า 11px · ห้ามตัดกลางคำ (ใช้ word segmentation — ดู 07)

## ระยะ ขนาด layout

- Base 4px: 4 · 8 · 12 · 16 · 20 · 24 · 32 · `pane-gap` 6
- **โครงแอป:** title bar 40 (touch 48) → toolbar 40 (58) → panes → status bar 24 (32)
- **แผง:** ตะกร้า 392 (touch 430) · หมวด 168 (touch → แถวปุ่ม) · แผงรายละเอียด 360–400 · ที่เหลือยืดตามจอ
- **Control:** 30 / แถว 30 / ปุ่มยืนยัน 44 (touch: 44 / 52 / 62) · hit target touch ≥ 44 ระยะห่าง ≥ 8
- ตัวเลขชิดขวา ข้อความชิดซ้าย · ปุ่มยืนยันอยู่ล่างสุดของแผงใน SaveBar

## รูปทรง ชั้น และเงา (soft — ค่าเริ่มต้น)

| องค์ประกอบ | radius | หมายเหตุ |
|---|---|---|
| หน้าต่าง | 12 | ถ้า OS อนุญาต |
| แผง, การ์ดสินค้า, การ์ดใน dashboard, ปุ่มเงินด่วน | 10 | แผงลอยบน `bg-canvas` เว้น `pane-gap` |
| ปุ่ม, ช่องกรอก, แท็บ, ปุ่ม toolbar, แถวที่เลือก | 8 | ไฮไลต์เว้นขอบ 6px |
| badge, count | 6 | |
| dialog, command palette, PIN | 14 | `shadow-3` |
| chip, จุด, avatar | full | |

เงาใช้น้อยมาก: `shadow-1` การ์ดบนแผง · `shadow-2` toast/popover · `shadow-3` dialog · **Square** (สำรอง): แผงชิดกันคั่น 1px, radius 3–4, แถวเลือกเป็นแถบเต็ม + เส้นซ้าย 2px

## Iconography

เส้น 1.8 บน grid 24 ปลายมน · 14 / 16 / 20 (touch) · อ้างอิง Lucide · icon ประกอบข้อความเสมอ ยกเว้นปุ่มที่รู้กันทั่วไป (ปิด, เพิ่มเติม, ลบในแถบแก้ไข) ซึ่งต้องมี label/tooltip

## Motion และ feedback

| เหตุการณ์ | motion | เสียง (ปิดได้) |
|---|---|---|
| เพิ่มสินค้าสำเร็จ | บรรทัดกะพริบ `accent-soft` 600ms · count เด้ง 300ms | ติ๊ดสั้นสูง |
| สินค้าหมด / ไม่พบบาร์โค้ด | — | ต่ำยาว + toast บอกวิธีแก้ |
| เปลี่ยนขั้นชำระเงิน / เปิดแผง | 200ms | — |
| hover/press | 120ms | — |

Reduced motion: ไม่มีการเคลื่อนไหว (สียังเปลี่ยนทันที) · ห้ามมี motion ที่ทำให้ต้องรอ

## Accessibility และการป้อนข้อมูล

Focus ring ฟ้า 2px · ช่องค้นหาบนหน้าขายได้ focus เสมอ (เครื่องสแกน) · ทุก action หลักมีคีย์ลัด · ไม่มีอะไรที่ต้อง hover/ดับเบิลคลิก/กดค้างเป็นทางเดียว · AccessKit label ครบ · UI scale 90–150%

## Localization

เผื่อข้อความอังกฤษยาวกว่าไทย ~30% · ชื่อสินค้าตัดบรรทัดได้ 2–3 บรรทัด (ชื่อเต็มใน tooltip/แผงรายละเอียด) · เงิน `฿1,461.35` ทศนิยม 2 ตำแหน่ง ลบใช้ `−` · วันที่ `14 ต.ค. 2569` / `14 Oct 2026` · เวลา 24 ชม. · แนะนำแยก `name` / `name_en` แทนการเขียน "ไทย / English" ในชื่อเดียว
