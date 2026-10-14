# 03 Components

## ชั้น

| ชั้น | สมาชิก |
|---|---|
| **Foundations** | tokens, Icon, type styles |
| **App chrome** *(ใหม่ v0.2)* | TitleBar + WorkspaceTabs · Toolbar (+ scope label, segmented ขวา) · Pane (+ head, body) · DetailsPane · SaveBar · StatusBar (+ items ที่กดได้) · CommandBar (Ctrl K) |
| **Primitives** | Button · IconButton · TextField · MoneyField · QuantityStepper · SearchField · Select · Checkbox · Switch · Segmented · Chip · Badge · Count · Dot · Avatar |
| **Composed** | ActionGroup · Combobox · FilterChips · DataTable + BulkBar · GroupedList (หัวกลุ่มตรึง) · Banner · Toast · Dialog · Sheet · Popover · StateView · StatTile · Chart · Steps (แนวตั้ง) · SettingsRow |
| **Domain** | ProductTile (+ แถบสีหมวด) · CartLine (+ LineEditor) · OrderSummary · PaymentPanel · StatusLine · DenominationCounter · ReceiptPreview · ActivityTimeline · TableCard · TaskItem · PinPad · KitchenTicket\* |

\* Future

## Catalog

| Component | การ์ด | สถานะ |
|---|---|---|
| TitleBar, Toolbar, Pane, SaveBar, StatusBar | Navigation | Ready |
| ทุกหน้าจอในสไตล์ v0.2 | **AppConcept** | Reference |
| Button, IconButton, ActionGroup | Button | Ready |
| TextField, MoneyField, Stepper, Search | TextField | Ready |
| Select, Combobox, Segmented, Chips | Select | Ready |
| DataTable, BulkBar | DataTable | Ready |
| ProductTile | ProductTile | Ready |
| CartLine | CartLine | Ready |
| OrderSummary / PaymentPanel | OrderSummary | Ready |
| Badge, StatusLine | StatusBadge | Ready |
| Banner, Toast | Feedback | Ready |
| Dialog, Sheet | Overlay | Ready |
| StateView | StateView | Ready |
| StatTile, Chart | StatTile | Experimental |
| PinPad, CommandBar, TaskItem, ActivityTimeline, DenominationCounter, TableCard | AppConcept | Experimental |

## เลือกอะไรเมื่อไร

- **ทำอะไรกับทั้งบิล/ทั้งหน้า** → ปุ่มใน Toolbar · **กับรายการเดียว** → LineEditor ใต้บรรทัด / แผงรายละเอียด
- **ขั้นต่อไปหลัก** → Button primary ใน SaveBar หรือท้าย OrderSummary (หนึ่งปุ่ม)
- **ดู/แก้รายการที่เลือกจากรายการยาว** → DetailsPane ด้านขวา (ไม่ใช่ dialog)
- **ฟอร์มตั้งค่า** → SettingsRow ในการ์ด + SaveBar เดียว
- **ยืนยันสิ่งที่ย้อนไม่ได้** → Dialog · **ย้อนได้** → ทำเลย + Toast "เลิกทำ"
- **ตัวกรองไม่กี่ค่า** → Chips · **สลับมุมมอง** → Segmented ใน Toolbar · **สลับแท็บในแผง** → แท็บเม็ดยาในหัวแผง

## กติกาการประกอบ

- หน้าขาย = Toolbar [บิลใหม่ | ทั้งบิล: พักบิล, ส่วนลดบิล, แยกบิล, ส่งครัว | สินค้า/โต๊ะ] → Pane ตะกร้า [แท็บ บิลปัจจุบัน/พักไว้ → ชื่อบิล → CartLines → OrderSummary → ส่งครัว + ชำระเงิน] → Pane หมวด (compact) → Pane สินค้า [Search → Chips (touch) → ProductTiles]
- หน้าจัดการ = Pane หมวด/มุมมอง → Pane ตาราง → DetailsPane [หัว + ตัวเลขสรุป + แท็บ + เนื้อหา + SaveBar]
- ห้าม: ปุ่ม primary ใน toast · dialog ซ้อน dialog · ปุ่มของรายการใน toolbar · การ์ดลอยมีเงาบนพื้นกระดาษ (ใช้เส้น `border-subtle` แทน)

**ไม่สร้าง component ใหม่ถ้า** ประกอบจากของเดิมได้ หรือใช้แค่หน้าเดียว (ยกระดับเมื่อใช้ ≥ 3 ที่)
