# 03 Components

## สถาปัตยกรรม 4 ชั้น

| ชั้น | ใช้ข้ามผลิตภัณฑ์? | สมาชิก |
|---|---|---|
| **Foundations** | ใช่ | tokens, Icon, type styles, Surface/Well |
| **Primitives** | ใช่ | Button · IconButton · TextField · MoneyField · QuantityStepper · SearchField · Select · Checkbox/Switch · Segmented · Chip · Tabs · Badge · Count · Dot |
| **Composed** | ใช่ | ActionGroup · Combobox · FilterBar · DateRange · Topbar (WorkspaceSwitcher + CommandBar + StatusPill) · Rail / SideNav · DataTable + BulkBar · Banner · Toast · Dialog · Sheet · Popover/Menu · StateView · StatTile · Chart · Steps |
| **Domain (เฉพาะงานร้านค้า)** | ใช้ได้ในครอบครัวผลิตภัณฑ์ร้านค้า | ProductTile · CartLine · OrderSummary / PaymentPanel · StatusLine (3 แกน) · DenominationCount · TableCard\* · KitchenTicket\* |

\* Future exploration — อยู่ในหน้าจอตัวอย่างเพื่อ stress-test เท่านั้น ยังไม่มีสเปกเต็ม

**กติกาของการแยกชั้น:** primitive และ composed ห้ามรู้จักแนวคิดของ POS (บิล, สินค้า, กะ) · domain component ประกอบจากชั้นล่างเท่านั้น และห้ามเพิ่ม token สีใหม่เอง

## Catalog

| Component | การ์ด | สถานะ | หน้าที่โดยย่อ |
|---|---|---|---|
| Button, IconButton, ActionGroup | Button | Ready | การกระทำ · มี 4 ระดับความเด่น, ขนาด sm–xl |
| TextField, MoneyField, QuantityStepper, SearchField | TextField | Ready | กรอกข้อมูล + validation |
| Select, Combobox, Segmented, FilterChips, DateRange | Select | Ready (DateRange: Experimental) | เลือกจากตัวเลือก |
| Topbar, WorkspaceSwitcher, CommandBar, Tabs, SideNav, Rail | Navigation | Ready (CommandBar: Experimental) | นำทางและสลับบริบท |
| DataTable, BulkBar, List | DataTable | Ready | ข้อมูลจำนวนมาก, เลือกหลายรายการ |
| ProductTile | ProductTile | Ready | เพิ่มสินค้าลงตะกร้าด้วยการแตะครั้งเดียว |
| CartLine | CartLine | Ready | บรรทัดในบิล + ตัวแก้ไข |
| OrderSummary / PaymentPanel | OrderSummary | Ready (จ่ายผสม: Future) | ยอดรวม รับเงิน เงินทอน |
| Badge, StatusLine | StatusBadge | Ready | สถานะ (ออเดอร์ / การชำระ / การทำอาหาร แยกกัน) |
| Banner, Toast, InlineError | Feedback | Ready | แจ้งผลและปัญหาพร้อมวิธีแก้ |
| Dialog, Sheet, Popover | Overlay | Ready | งานที่ต้องแยกออกจากหน้าหลัก |
| StateView (empty / loading / error / offline) | StateView | Ready | สถานะของพื้นที่เนื้อหา |
| StatTile, Chart, Legend | StatTile | Experimental | สรุปตัวเลขและกราฟ |
| Icon | Icon | Ready | ชุด icon เส้น |

## เลือก component อย่างไร (guide)

**ต้องการให้ผู้ใช้ทำอะไรบางอย่าง →**
- เป็นขั้นต่อไปหลักของหน้า → `Button primary` (หนึ่งปุ่ม) · ถ้าเป็นการรับเงิน → ขนาด `xl` + คีย์ลัด
- ทางเลือกรอง → `Button` (secondary) · ยกเลิก/ปิด → `ghost`
- ทำลายข้อมูล ย้อนกลับไม่ได้ → `danger` + Dialog · ย้อนกลับได้ → `danger-quiet` + Toast ที่มี “เลิกทำ”

**ต้องการให้เลือก →**
- 2–5 ตัวเลือก เห็นพร้อมกันได้ ผลเกิดทันที → `Segmented`
- เปิด/ปิดตัวกรองหลายอัน → `Chip` ใน `FilterBar`
- ตัวเลือกยาวหรือไม่จำกัด → `Select` (≤ 15) / `Combobox` (ค้นหาได้, สร้างใหม่ได้)
- เปิด/ปิดการตั้งค่า → `Switch` (มีผลทันที) · `Checkbox` (มีผลเมื่อกดบันทึก หรือใช้เลือกแถว)

**ต้องการแสดงสถานะ →** ค่าคงที่ของสิ่งนั้น → `Badge` · ผลของการกระทำเมื่อครู่ → `Toast` · ปัญหาที่ยังค้างอยู่และต้องทำอะไรต่อ → `Banner` · ปัญหาของช่องกรอกช่องเดียว → `InlineError` · ทั้งพื้นที่ว่าง/โหลด/ผิดพลาด → `StateView`

**ต้องการพื้นที่เพิ่ม →** ตัดสินใจสั้น ๆ หรือยืนยัน → `Dialog` · แก้ไขรายการโดยยังเห็นบริบทเดิม → `Sheet` (ด้านข้างบนเดสก์ท็อป, ด้านล่างบน tablet) · ตัวเลือกเล็ก ๆ ใกล้ปุ่ม → `Popover` · งานหลายขั้นตอนหรือยาว → หน้าใหม่ + `Steps`

## กติกาการประกอบ (composition)

- **ตะกร้า** = Surface → Tabs (บิลปัจจุบัน/พักไว้) → หัวบิล (title + IconButton) → รายการ CartLine (เลื่อนได้, ค่อย ๆ จางที่ขอบล่าง) → OrderSummary (ตรึงไว้) → chip เงินด่วน → Button xl
- **หน้าจัดการ** = Topbar → SideNav (มุมมองที่บันทึกไว้) → Surface [หัวหน้า + action] → FilterBar → BulkBar (เมื่อมีการเลือก) → DataTable → Inspector/Sheet
- **ห้าม** เอาปุ่ม primary ไปไว้ใน Toast/Banner · ห้ามซ้อน Dialog บน Dialog (ใช้ขั้นตอนใน Dialog เดียวแทน) · ห้ามใช้ Badge แทนปุ่ม

## ไม่สร้าง component ใหม่ถ้า…

- ประกอบจากของเดิมได้ (เช่น “การ์ดสรุปกะ” = Surface + `ds-kv` + Badge)
- ต่างกันแค่เนื้อหา ไม่ได้ต่างที่พฤติกรรม
- ใช้แค่หน้าจอเดียว → ทำเป็น pattern ในหน้านั้นก่อน ถ้ามีหน้าจอที่ 3 ต้องใช้ ค่อยยกระดับ (ดู 06 Governance)
