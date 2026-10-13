# POS Heart

<!-- impeccable:product-schema 1 -->

## Platform
Native desktop: macOS and Windows. Not a web application.

## Stack
Confirmed: Rust, egui and eframe. SQLite via Diesel ORM with bundled SQLite; bound raw SQL for migrations, PRAGMAs, reports and backups. User confirmed: pure Rust applies to application/UI, not all third-party dependencies.

## Users
เจ้าของร้านและพนักงานขายของร้านค้าทั่วไป ร้านอาหารแบบเคาน์เตอร์หรือมีโต๊ะ และร้านผสม

## Product Purpose
ขายสินค้าและอาหารในแอปเดียว ใช้งานออฟไลน์ เก็บข้อมูลการขายถาวร และเปิดกลับมาใช้ต่อได้

## Operating Context
เวอร์ชันแรก: เครื่องขายเดียว คีย์บอร์ด เมาส์ และจอสัมผัส ภาษาไทย/English ธีมสว่าง มืด และตามระบบ ตะกร้า ยอดรวม รับเงินและเงินทอนฝั่งซ้าย รายการสินค้าและค้นหาฝั่งขวา (ผู้ใช้ยืนยันแล้ว)

## Capabilities and Constraints
- เงินบาท เก็บเป็นจำนวนเต็มหน่วยสตางค์ ไม่ใช้ floating point
- สินค้าที่ติดตามสต็อกและเมนูที่ไม่ติดตามสต็อกอยู่ในบิลเดียวกันได้
- เป้าหมายรอบแรก: จัดการสินค้า ตะกร้า หมายเหตุ พักบิล รับเงินสด ประวัติการขาย
- ส่วนลดรายการหรือทั้งบิล เป็นบาทหรือเปอร์เซ็นต์ พร้อมเหตุผล (ไม่บังคับ) คิดใหม่ตอนรับเงิน
- โอนเงินแสดง QR พร้อมเพย์พร้อมยอดได้เมื่อตั้งค่าพร้อมเพย์ไว้ ยังบันทึกเป็นเงินโอน คนขายยืนยันเองว่าเงินเข้าแล้ว
- สินค้ามีกลุ่มตัวเลือกได้ (เช่น ความหวาน ท็อปปิ้ง) ตัวเลือกบวกราคาเพิ่มต่อชิ้นได้ ราคาคิดจากรายการสินค้าตอนรับเงิน
- ตัวโปรแกรมและ UI เขียน Rust; ยอมรับ SQLite ที่มีแกน C ตามที่ผู้ใช้ยืนยัน
- UI รองรับไทยและ English สลับภาษาได้; ข้อมูลสินค้าและหมายเหตุไม่แปลอัตโนมัติ
- โต๊ะ/โซนและจอครัว/ใบสั่งครัวทำแล้ว (ดู Tables and kitchen ด้านล่าง)
- แบ่งจ่าย: บิลเดียวจ่ายหลายวิธีได้ (เงินสด + โอน + บัตร) ยอดแต่ละส่วนต้องรวมเท่ายอดชำระพอดี ไม่ปัดเศษเงินสด เฉพาะส่วนเงินสดเข้าลิ้นชัก รายงานแยกตามวิธี ยกเลิกบิลคืนแต่ละส่วนตามวิธีเดิม คืนบางรายการเลือกวิธีที่บิลจ่ายได้ไม่เกินยอดของวิธีนั้น
- แยกบิล (จ่ายแยก): ย้ายบางรายการไปบิลใหม่เพื่อจ่ายต่างหาก รายการที่ส่งครัวแล้วไม่ถูกส่งซ้ำหรือยกเลิก บิลที่เหลือยังพักไว้หรืออยู่ที่โต๊ะเดิม
- ยังไม่รวมวัตถุดิบตามสูตร หลายเครื่อง ซิงก์ออนไลน์ ใบลดหนี้ ยื่นแบบภาษี โปรโมชันอัตโนมัติ/คูปอง ตรวจยอดโอนกับธนาคาร ใบสั่งซื้อ/ผู้ขาย (ใน design system เป็น Future) ข้อเสนอเรื่องหลายเครื่องสำหรับตัดสินใจภายหลัง: [docs/multi-device.md](docs/multi-device.md)
- ไม่อ้างว่าใช้หน้างานจริงได้จนกว่าจะทดสอบ flow และเคสผิดพลาดที่เกี่ยวข้อง

## Product Principles
- ขั้นตอนขายสั้นและจำนวนเงินอ่านชัด
- บันทึกบิลและตัดสต็อกต้องสำเร็จพร้อมกัน
- รายการขายเก็บชื่อและราคาขณะขาย ไม่เปลี่ยนตามการแก้สินค้า
- ข้อมูลตัวอย่างต้องเลือกโหลดและแยกจากข้อมูลร้านจริง
- แยกกฎธุรกิจออกจากหน้าจอและฐานข้อมูล

## Open Decisions
อุปกรณ์ปลายทาง, หลายเครื่อง (เลื่อนออกไป ดู docs/multi-device.md), ต้นทุนตามสูตรหรือถัวเฉลี่ย

## Current implementation priority
The current iteration adds customizable print templates, an on-screen print preview and a check bill, on top of tables/zones, kitchen tickets with a kitchen screen and kitchen printing (schema 15), recovery folders, CSV product imports, the activity journal and standard product costs. Multi-device use is postponed (see docs/multi-device.md). Keep the existing offline single-register workflows, immutable receipts and per-command permissions. Physical devices, Windows runtime and operational recovery still need field validation.

## Full void and daily reporting
- Cancel a whole paid sale with a required reason and recorded original-method refund equal to the original sale total. Original receipt remains immutable; stock and audit record commit together, once per sale.
- New receipts remember whether each line deducted stock. Legacy receipts require explicit operator stock review rather than guessing from today’s catalog.
- Reports select a Bangkok date (UTC+7): sales by payment time, refunds by cancellation time, gross/refund/net amounts, counts and top 10 by net units. Refunds of earlier receipts can make a day negative.
- Reports exclude opening float and non-sale drawer movements. This is not a shift reconciliation or tax report.

## Shop identity and startup
There is no shop type: every shop has the same retail/food capabilities (the former descriptive type was removed from the UI on 2026-10-13; `shop.json` keeps its field for compatibility). Home shows actual Bangkok-today gross/refund/net sales and held bills, alongside separately labeled active-shift drawer totals. Shop profile and device startup preferences persist separately from sales, in versioned shop.json. No account is required. New payments and refunds require an open shift. Closing records a permanent expected/counted/difference snapshot; multi-register management remains outside this increment (optional staff sign-in was added later, see below).


## Payment-method increment
Cash / Transfer / Card, one method per bill. Transfer and card are manually verified external payments recorded offline, with an explicit amount review. No automatic bank checks, charging, terminal integration, card data storage or split tender. Refunds return the full original total through the original method. Daily reporting distinguishes gross/refund/net by method; drawer expected/count discrepancy includes cash only. Existing visual system and left-payment/right-products layout remain in force.


## Design system v0.1 redesign
The whole desktop UI follows the user's POS Design System v0.1 (`design/`, "Calm counter"): warm stone surfaces, one saffron accent for money-moving commits, IBM Plex type with monospaced money, Compact/Touch density, workspace switcher and `Ctrl K` command palette, undo toasts for reversible actions. The user chose to implement the reference screens' features as well: category column and saved product views, barcode and on-sale switch, bulk actions and CSV export, product options with prices, held-bill tab with autosaved current bill, quick cash amounts, steps-based shift close with denomination count and difference reason, hourly report with last-week comparison. Items the design system itself marks Future (tablet layout, split tender, multi-branch) are not built; tables and the kitchen display were added later (see Tables and kitchen).

## Shop management increment (2026-10-13)
Stock receive / remove / count with history, period reports (up to 366 days, categories, previous-period comparison) and partial refunds by item through the original payment method.

## Discounts and PromptPay QR (2026-10-13)
ส่วนลดต่อรายการ (F7) และทั้งบิล (F8) เป็นบาทหรือเปอร์เซ็นต์ พร้อมเหตุผลร่วมของบิล ส่วนลดทั้งบิลกระจายลงแต่ละรายการตามสัดส่วน คืนเงินบางรายการคืนตามยอดที่จ่ายจริง รายงานแสดงยอดที่รับจริงและส่วนลดที่ให้ PromptPay ID ใน Settings → Shop ทำให้หน้าตรวจรับเงินโอนแสดง QR พร้อมยอด ยังเป็นการบันทึกเงินโอนแบบยืนยันเอง แอปไม่เชื่อมธนาคาร No coupons, automatic promotions or bank verification; discount approvals were added in the Staff increment below.

## Receipt printing (2026-10-13)
พิมพ์ใบเสร็จและสรุปกะกับเครื่องพิมพ์ความร้อน ESC/POS 58/80 มม. ผ่านเครือข่ายในร้าน (LAN) หรือเครื่องพิมพ์ในระบบ (macOS/Linux) ภาษาไทยพิมพ์เป็นภาพ พิมพ์อัตโนมัติหลังรับเงินได้ เปิดลิ้นชักเมื่อรับเงินสดได้ พิมพ์สำเนาจากประวัติการขาย ตั้งค่าเฉพาะเครื่องใน printer.json พิมพ์ไม่สำเร็จไม่กระทบบิล Not yet tested with a physical printer or cash drawer; Windows supports network printers only. VAT receipts and full tax invoices print too (see below).

## Staff sign-in and approvals (2026-10-13)
พนักงานเข้าใช้ด้วย PIN (ไม่บังคับ ร้านที่ไม่มีพนักงานใช้งานเหมือนเดิม) บทบาทตั้งต้น เจ้าของ / ผู้จัดการ / แคชเชียร์ (ปรับและเพิ่มบทบาทเองได้ ดู Custom roles) แคชเชียร์ต้องให้ผู้จัดการใส่ PIN อนุมัติเมื่อคืนเงิน ยกเลิกบิล นำเงินเข้า/ออกลิ้นชัก แก้สินค้าหรือสต็อก และให้ส่วนลดเกินเพดาน (ค่าเริ่มต้น 10%) บิล การคืนเงิน กะ และการเคลื่อนไหวเงิน/สต็อกบันทึกชื่อผู้ทำและผู้อนุมัติ รายงานแยกตามพนักงาน PIN ใช้ระบุตัวคนที่เคาน์เตอร์ ไม่ได้เข้ารหัสฐานข้อมูล Schema 9: older builds cannot open an upgraded database. No auto-lock timer, shift-per-employee scheduling or payroll.

## VAT and tax invoices (2026-10-13)
ร้านที่จด VAT ตั้งค่าใน Settings → VAT (บทบาทที่อนุญาต Settings): ชื่อผู้ประกอบการ เลขประจำตัวผู้เสียภาษี (ตรวจหลักสุดท้าย) สำนักงานใหญ่/สาขา ที่อยู่ อัตรา (7%) และเลขเครื่อง POS ราคารวม VAT แล้ว (หรือบวก VAT เพิ่ม ดูด้านล่าง) ใบเสร็จที่รับเงินหลังเปิด VAT เป็นใบกำกับภาษีอย่างย่อ ออกใบกำกับภาษีเต็มรูปได้ใบเดียวต่อบิล เลขรันแยก บิลที่คืนเงินแล้วออกไม่ได้เพราะต้องใช้ใบลดหนี้ซึ่งยังไม่มี รายงานแสดง VAT ขายและ VAT คืน ต้องได้รับอนุมัติจากกรมสรรพากร (ภ.พ.06) ก่อนใช้ออกใบกำกับภาษีอย่างย่อ Schema 10: older builds cannot open an upgraded database. No credit notes, VAT return filing or e-Tax; layout not yet reviewed by an accountant.

## Service charge, ++ prices and cash rounding (2026-10-13)
ค่าบริการ (%) ตั้งได้ใน Settings → Pricing ยกเว้นได้รายสินค้าและต่อบิล (กลับบ้าน) ราคาสินค้าเลือกได้ว่ารวม VAT หรือบวก VAT เพิ่ม (++) ปัดเศษเฉพาะเงินสด 0.25 / 0.50 / 1 บาท ค่าเริ่มต้นปัดลงให้ลูกค้า ลำดับคิดราคา: ส่วนลด → ค่าบริการ → VAT → ปัดเศษ VAT ในใบเสร็จคิดจากยอดที่จ่ายจริง ใบเสร็จและรายงานแสดงค่าบริการและยอดปัดเศษ ไม่อ้างกฎหมายการปัดเศษ ควรยืนยันกับนักบัญชี Schema 11: older builds cannot open an upgraded database.

## Custom roles (2026-10-13)
เจ้าของร้านกำหนดบทบาทเองได้ สิทธิ์ 10 อย่าง (คืนเงินบางรายการ ยกเลิกทั้งบิล ส่วนลดเกินเพดาน เงินเข้า/ออก ปิดกะ สินค้า สต็อก รายงาน ตั้งค่า จัดการพนักงาน) ตั้งได้ว่า อนุญาต / ต้องอนุมัติ / ไม่อนุญาต พร้อม "อนุมัติให้คนอื่นได้" และเพดานส่วนลดต่อบทบาท บทบาทเจ้าของคงที่ ผู้จัดการและแคชเชียร์แก้ได้แต่ลบไม่ได้ อนุมัติให้ตัวเองไม่ได้ และสิทธิ์ที่ไม่อนุญาตอนุมัติแทนไม่ได้ Schema 12: older builds cannot open an upgraded database.

## Recovery and shop operations (2026-10-13)

Complete recovery folders include checksummed business data and shop/cart/drawer sidecars. Review before confirmation, activate on next launch, retain a restorable safety backup and the original directory; printer/display settings stay on the device. CSV import previews and atomically creates new products without overwriting stock. Unified activity preserves actor/approver snapshots from this version onward. Optional standard unit cost is snapshotted at payment; estimated gross profit separates unknown-cost revenue, excludes VAT and reverses on refund day. No inventory cost averaging or operating-expense accounting. Schema 14.

## Tables and kitchen (2026-10-13)

ร้านที่มีโต๊ะตั้งโซนและโต๊ะได้ใน Settings → Tables & zones (ไม่ตั้งก็ใช้งานเหมือนเดิม) ผังโต๊ะอยู่ในหน้าขาย (แท็บ สินค้า / โต๊ะ, `Alt 9`) แสดงโต๊ะตามโซน ว่าง/มีลูกค้า ยอด จำนวนแขก และเวลารอ แตะโต๊ะเพื่อเปิดบิลทางซ้าย ส่งครัว รับเงิน หรือพักบิลโต๊ะแล้วกลับมาที่ผังโต๊ะ มีปุ่มขายหน้าร้าน/กลับบ้าน ตั้งจำนวนคนที่หัวบิล ย้ายโต๊ะจากเมนู ⋯ หนึ่งโต๊ะมีบิลเปิดได้บิลเดียว แท็บพักไว้แสดงเฉพาะบิลหน้าร้าน ส่งเข้าครัว (`F10`) ส่งเฉพาะที่เพิ่มหรือยกเลิกตั้งแต่ส่งครั้งก่อน การยกเลิกจานที่ครัวรับแล้วบันทึกใน Activity สินค้ามีสวิตช์ "ส่งเข้าครัว" จอครัวเปิดเป็นหน้าต่างที่สองสำหรับจออีกตัว (ติ๊กจานที่ทำแล้ว กดเสร็จ เรียกคืนได้) พิมพ์ใบสั่งครัวได้ที่เครื่องพิมพ์ใบเสร็จหรือเครื่องพิมพ์ครัวแยก รับเงินแล้วโต๊ะว่างทันที No multi-device ordering, reservations, per-seat bill splitting or course timing. Schema 15: older builds cannot open an upgraded database. Not yet tested on a real second monitor, kitchen printer or Windows.

## Print templates, preview and check bill (2026-10-13)

Settings → Printed documents ปรับหน้าตาใบเสร็จ ใบแจ้งยอด (check bill) และใบสั่งครัวได้ พร้อมตัวอย่างสดของสิ่งที่จะพิมพ์จริง: ชื่อหัวใบเสร็จ ข้อความหัว/ท้าย โลโก้ (PNG/JPEG แปลงเป็นขาวดำ) ขนาดตัวอักษร ภาษา และเลือกแสดงชื่อบิล พนักงาน ราคาต่อหน่วย ตัวเลือก หมายเหตุ SKU เงินสด/เงินทอน และ QR พร้อมเพย์บนใบแจ้งยอด ใบเสร็จ VAT และใบกำกับภาษีเต็มรูปยังพิมพ์ทุกบรรทัดที่กฎหมายกำหนดเสมอ ใบสั่งครัวพิมพ์หมายเหตุและการยกเลิกเสมอ ดูตัวอย่างก่อนพิมพ์ได้จากใบเสร็จ ประวัติการขาย และหน้าชำระเสร็จ แม้ไม่มีเครื่องพิมพ์ เมนู More ในหน้าขายมี Check bill: ใบแจ้งยอดก่อนจ่าย คิดราคาแบบเดียวกับตอนชำระ ไม่ใช่ใบเสร็จและไม่บันทึกอะไร โลโก้ถูกรวมใน recovery backup No schema change. Not yet tested on printer hardware or Windows.
