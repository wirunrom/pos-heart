# 06 Governance

ใช้กับทีมเล็กได้: เรียบง่าย แต่ป้องกันไม่ให้ระบบซ้ำซ้อน

## เพิ่ม component หรือ variant

ก่อนเพิ่ม ให้ตอบ 4 ข้อนี้ใน PR/issue:

1. **ปัญหาคืออะไร** — มีหน้าจอจริงที่ต้องการกี่หน้า? (กติกา: **ใช้ ≥ 3 ที่** หรือเป็น domain component ที่สำคัญ)
2. **ประกอบจากของเดิมได้ไหม** — ถ้าได้ ทำเป็น pattern/recipe ในเอกสารแทน
3. **เป็น variant หรือ component ใหม่** — ต่างที่*หน้าตา*อย่างเดียว → variant · ต่างที่*พฤติกรรมหรือโครงสร้าง* → component ใหม่
4. **อยู่ชั้นไหน** — primitive / composed / domain (ดู 03) · ถ้าต้องรู้จักแนวคิด POS → domain

**Definition of done:** README (ใช้เมื่อไร/ไม่ควรใช้/anatomy/states/keyboard/a11y/overflow) · preview Light+Dark, ไทย+อังกฤษ, compact+touch · ใช้แค่ semantic/component tokens · ผ่าน checklist ด้านล่าง · มี mapping ใน egui (หรือบอกว่ายังไม่มี)

## การตั้งชื่อ

- Component: PascalCase ตามหน้าที่ (`CartLine`, ไม่ใช่ `SellLeftRow`) · ใน egui: `cart_line(ui, &CartLineProps)` หรือ `CartLine::new(..).show(ui)`
- Variant: คำคุณศัพท์สั้น ๆ (`primary`, `ghost`, `danger`) · ขนาด: `sm md lg xl` · สถานะ: `is-current`, `is-unavailable`
- Token: ดู 02 Tokens · ห้ามมีชื่อผลิตภัณฑ์, สี หรือชื่อหน้าจอในชื่อ

## Versioning (SemVer แบบเบา)

| เปลี่ยน | ระดับ | ตัวอย่าง |
|---|---|---|
| แก้ค่าโดยความหมายเดิม | patch `0.1.x` | ปรับ `stone-600` ให้ contrast ดีขึ้น |
| เพิ่ม token/variant/component | minor `0.x.0` | เพิ่ม `Switch` |
| เปลี่ยนชื่อ, ลบ, เปลี่ยนความหมาย | major `x.0.0` (ระหว่าง 0.x ใช้ minor + หมายเหตุ **Breaking**) | `accent-bg` เปลี่ยนจาก “ปุ่มหลัก” เป็น “ตกแต่ง” |

`tokens.json → version`, `PosDS.version` และ CHANGELOG ต้องตรงกัน

## Deprecation

1. ทำเครื่องหมาย `deprecated` ใน README/usage ของ token + บอกตัวที่ใช้แทน + เวอร์ชันที่จะลบ
2. ใน egui: `#[deprecated(note = "use semantic::bg_raised")]`
3. เก็บไว้อย่างน้อย 1 minor version แล้วจึงลบ
4. บันทึกใน CHANGELOG ทั้งตอนประกาศและตอนลบ

## CHANGELOG

```
## 0.2.0 — 2026-10-14
### Changed   Single theme (graphite chrome + paper panes); Light/Dark removed. Soft shape default (panes 10, controls 8).
              App chrome: workspace tabs in title bar, contextual toolbar with scope, status bar with actions.
              Payment is a two-step flow. Disabled primary = grey + reason. Line actions moved into LineEditor.
### Added     Tokens: bg-chrome*, fg-chrome*, border-chrome, cat-*, pane-gap, radius-window, titlebar/toolbar/statusbar sizes.
              Components: TitleBar, Toolbar, Pane, SaveBar, StatusBar, CommandBar, PinPad, ActivityTimeline, DenominationCounter, TableCard.
              Feedback cues (flash, count bump, sounds). Pinned products. Accent presets.
### Removed   v0.1 screen cards (ScreenSell, ScreenProducts, ScreenShiftClose, ScreenTableOrders, ScreenReports), PrototypeSell → see AppConcept.
              Tokens: per-theme values, stone-* / old saffron ramp names.
```

## CHANGELOG (รูปแบบ)

```
## 0.2.0 — 2026-11-02
### Added     Switch (primitive). size.touch-target-min token.
### Changed   ProductTile: name clamp 2 → 3 lines (Thai long names).
### Deprecated  bg-raised-hover → use bg-hover. Removal in 0.4.0.
### Fixed     fg-muted on bg-sunken (dark) 4.3 → 4.8:1.
```

## Checklist ก่อน merge

**Accessibility**
- [ ] ข้อความ ≥ 4.5:1 · ขอบ control/focus/icon สำคัญ ≥ 3:1 (ทั้งสองธีม — สคริปต์ตรวจ contrast อยู่ใน pipeline ของ token)
- [ ] ใช้ได้ด้วยคีย์บอร์ดอย่างเดียว · focus มองเห็นได้ · ลำดับ focus สมเหตุสมผล · `Esc` ปิด overlay
- [ ] เป้าการกดในโหมด touch ≥ 44×44 · ไม่มีอะไรที่ต้อง hover ถึงจะใช้ได้
- [ ] ไม่พึ่งสีอย่างเดียว · icon-only มี label · AccessKit role/label ครบ

**Density & shape parity**
- [ ] ตรวจทั้ง compact และ touch: เป้ากด touch ≥ 44, ไม่มีอะไรถูกตัด, คำใบ้คีย์ลัดซ่อนใน touch
- [ ] ไม่มีค่าสีตรง ๆ ในโค้ด (grep หา `#`/`Color32::from_rgb` ใน widget) · ปุ่มที่กดไม่ได้บอกเหตุผล

**Localization**
- [ ] ทดสอบไทยและอังกฤษด้วย*ข้อความยาวจริง* (ชื่อสินค้ายาว 80 ตัวอักษร, ปุ่มภาษาอังกฤษ)
- [ ] เงิน: ครบทศนิยม, ไม่ตัด, ชิดขวา, ใช้ `font-num` · วันที่/เวลาผ่าน formatter
- [ ] สระ/วรรณยุกต์ไทยไม่ถูกตัดด้านบน/ล่าง (line-height) ที่ 12/14/16px

**Visual consistency**
- [ ] ตรวจที่ 1000×700 และ 1280×820 · compact และ touch
- [ ] มีปุ่ม accent ไม่เกิน 1 ปุ่มต่อมุมมอง · ระยะทั้งหมดเป็น `space-*`
- [ ] เทียบกับหน้าจอตัวอย่างที่ใกล้เคียงที่สุด

## สถานะของระบบตอนนี้ (v0.2)

| พร้อมใช้ (Ready) | ทดลอง (Experimental) | ต้องพิสูจน์เพิ่ม |
|---|---|---|
| Tokens v0.2, app chrome, primitives, Feedback, Overlay, DataTable, ProductTile, CartLine, OrderSummary, ชำระ 2 ขั้น | เสียง, Ctrl K, PIN, หมวดแถวปุ่ม, ชุดสีหลัก, StatTile/Chart | Thai shaping/ตัดคำใน egui · unified title bar บน Windows · เป้ากดบนจอจริง · KDS/จอลูกค้า v0.2 |

**ผู้ดูแล:** ตอนนี้ 1 คน (ผู้ดูแลผลิตภัณฑ์) อนุมัติการเปลี่ยนแปลงทั้งหมด · ทบทวนระบบทุกครั้งที่เพิ่มฟีเจอร์ใหญ่ หรือทุก 3 เดือน
