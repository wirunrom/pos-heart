# จัดซื้อและรับของ

Mode: Operate. Native Rust/egui desktop extension to Products → Manage → Purchasing, using the existing POS Design System v0.1, DESIGN.md and desktop.md. No new visual identity or tokens. Source: `apps/pos-desktop/src/purchasing_ui.rs` and its worker commands; data rules: docs/architecture.md "Purchasing and receiving".

## Purpose and first view

ช่วยเจ้าของร้านบันทึกสั่งสินค้า/วัตถุดิบ แล้วรับจริงทีละบางส่วนโดยเห็นจำนวนที่สั่ง รับแล้วและคงเหลือ เปิดจากกลุ่มจัดการของหน้าสินค้า ใช้พื้นที่รายละเอียดเดิม หน้าจอมีหัวข้อ จัดซื้อและรับของ, Reload และแท็บ Orders / Suppliers แยกข้อมูลสั่งกับข้อมูลผู้ขาย รายการสั่งใหม่อยู่บนสุดและมีหน้าละ 50; สถานะรอรับ รับบางส่วน รับครบ หรือยกเลิกส่วนที่เหลือสื่อด้วยข้อความพร้อมสี semantic

## Orders and receiving

ใบสั่งซื้อเก็บผู้ขายและชื่อ/หน่วย/ราคาของรายการตอนสั่ง แก้ผู้ขายหรือแค็ตตาล็อกภายหลังไม่เปลี่ยนชื่อย้อนหลัง สร้างใบสั่งซื้อเลือกผู้ขาย เลือกสินค้าที่ติดตามสต็อกหรือวัตถุดิบ ใส่จำนวน ราคาต่อหน่วยและหมายเหตุ แล้วตรวจอีกครั้งก่อนสร้าง สินค้าเป็นจำนวนเต็ม วัตถุดิบทศนิยมได้สามตำแหน่ง

รายละเอียดใบสั่งซื้อแสดงผู้ขาย สถานะ รายการ จำนวนสั่ง/รับแล้ว/คงเหลือ ราคาต่อหน่วย ยอดสั่งและมูลค่ารับสะสม พร้อมประวัติใบรับที่แบ่งหน้าได้ รับของเปิดร่างจำนวนแต่ละรายการ ช่องว่างหมายถึงไม่รับรายการนั้นรอบนี้ ยอดแสดงก่อนยืนยันใช้ส่วนต่างของมูลค่าสะสมที่ปัดสตางค์แล้ว เช่น รับวัตถุดิบเศษหน่วยหลายครั้งรวมกันต้องเท่ายอดสั่งเมื่อรับครบ คำสั่ง Receive/Confirm receiving เพิ่มสต็อกและบันทึกใบรับพร้อมกัน ใช้ neutral controls ทั้งตรวจทานและยืนยัน เพราะเป็นการรับสต็อก ไม่มีธุรกรรมเงิน; saffron สงวนไว้สำหรับรับชำระเงินจริง

หาก revision เปลี่ยน หน้ารับ/ยกเลิกให้ Reload and review และต้องตรวจใหม่ ห้ามยืนยันจากยอดที่เคยตรวจไว้ ยกเลิกส่วนที่ยังไม่รับต้องมีเหตุผลและยืนยันด้วย destructive styling ของระบบ ของที่รับแล้วและประวัติใบรับยังอยู่ ไม่มีการคืนสต็อกย้อนหลัง

## Suppliers and drafts

แท็บผู้ขายแสดงชื่อและข้อมูลติดต่อ เพิ่ม/แก้ชื่อ ติดต่อ หมายเหตุและ active ได้ การปิดผู้ขายไม่ลบประวัติใบสั่งซื้อ ร่างใบสั่งซื้อ ผู้ขาย รับของและเหตุผลยกเลิกคงอยู่เมื่อล็อกจอ และต้องบันทึกหรือทิ้งร่างก่อนเปลี่ยน workspace Close/recovery ตรวจร่างก่อนทิ้ง Lock ซ่อน dialogs และล้าง confirmation; หลังเข้าใช้ต้องตรวจใหม่ สิทธิ์ของคนใหม่กำหนดว่าเปิดร่างใดต่อได้

Catalog ใช้เพิ่ม/แก้ผู้ขาย สร้างใบสั่งซื้อและยกเลิก; Stock ใช้รับของ Allow และ Approval แสดง action ได้ ส่วน Deny ซ่อน/ปิดเส้นทางที่เกี่ยวข้อง หน้ารวมเปิดได้เมื่อมี Catalog หรือ Stock งานเขียนยังตรวจสิทธิ์ใน worker transaction และใช้ flow อนุมัติ PIN ครั้งเดียวของระบบ

## Visual and interaction rules

ใช้ Theme palette, metrics, spacing และ type scale เดิม เงินผ่าน `ui::format` เป็นเลขสองตำแหน่งใน IBM Plex Mono จัดขวา ปริมาณใช้ formatter thousandths ของระบบพร้อมหน่วย ข้อความไทยและ English สลับตามภาษาหน้าจอ ชื่อผู้ขาย/รายการเป็นข้อมูลร้าน ไม่แปลอัตโนมัติ Dialog body เลื่อนได้และแยก action footer; ปุ่มสำคัญมีข้อความ ใช้ได้ทั้งเมาส์ คีย์บอร์ดและ touch density Error อยู่ใน flow พร้อมร่างเดิมเพื่อแก้ไขหรือโหลดใหม่

Help text อธิบายว่ารับของเพิ่มสต็อกเท่านั้น ไม่มีการจ่ายเงิน เจ้าหนี้ VAT จัดซื้อ ค่าขนส่ง คืนผู้ขาย หรือติดต่อผู้ขาย ไม่เปลี่ยนต้นทุนมาตรฐานสินค้าและต้นทุนต่อหน่วยวัตถุดิบอัตโนมัติ จึงแยกมูลค่าการรับออกจากกำไรขายและยอดลิ้นชัก

## Checks and captures

ตรวจ native ที่ 1280×820 / 1000×700, Light/Dark, ไทย/English, Compact/Touch; รวม focus, scroll, review/confirmation, stale reload, role/approval, lock และ close/recovery draft guards แยกผลกดจริงออกจาก debug preview; Windows และ hardware ยังต้องมีหลักฐานแยก

ข้อมูลจำลอง: `cargo run -p pos-desktop --example seed_screens -- DIR --purchases` (เพิ่ม `--staff` ได้) ใช้ `--page products --preview purchasing`, `supplier-editor`, `purchase-create`, `purchase-create-confirm`, `purchase-detail`, `purchase-receive`, `purchase-receive-confirm`, `purchase-cancel`; prefix `as-owner:` เมื่อเปิด staff ภาพ preview ยืนยัน layout เท่านั้น ผล runtime และข้อจำกัดล่าสุดอยู่ใน docs/validation.md
