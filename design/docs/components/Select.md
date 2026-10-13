# Select
ตัวเลือกแบบต่าง ๆ: Select, Combobox (ค้นหา/สร้างใหม่ได้), Segmented, FilterChips และ DateRange · **Ready** (DateRange: Experimental)

| ใช้ | เมื่อ |
|---|---|
| **Segmented** | 2–5 ตัวเลือกที่เห็นพร้อมกันได้และมีผลทันที (ช่วงเวลารายงาน, ความหวาน) |
| **Select** | ≤ 15 ตัวเลือก รายการคงที่ |
| **Combobox** | รายการยาว/เพิ่มได้ (ผู้ขาย, ลูกค้า) · ไฮไลต์คำที่ตรง · มีแถว “เพิ่ม ‘…’” ท้ายรายการ |
| **FilterChips** | ตัวกรองแบบเปิด/ปิด ใน FilterBar · chip ที่เปิดแสดงจำนวนผล · มี “ล้างตัวกรอง” เสมอ |
| **DateRange** | Segmented สำหรับช่วงที่ใช้บ่อย + “กำหนดเอง” เปิดปฏิทินใน Popover · แสดงตามภาษา (พ.ศ./ค.ศ.) |

**ไม่ควรใช้** Select กับ 2–3 ตัวเลือก (ใช้ Segmented) · ห้ามใช้ Segmented แทน Tabs ที่เปลี่ยนเนื้อหาทั้งหน้า
**Keyboard** Select/Combobox: `↑↓` เลื่อน, `Enter` เลือก, `Esc` ปิด, พิมพ์เพื่อกระโดด · Segmented: `←→` เลื่อนและเลือก · Chips: `Space` เปิด/ปิด
**Touch** popover เต็มความกว้างของช่อง, แถว ≥ 48px · บน tablet รายการยาวเปิดเป็น bottom sheet
**Overflow** ตัวเลือกยาวตัดบรรทัดในรายการได้ แต่ในช่องที่ปิดอยู่ให้ ellipsis + tooltip
**egui** `ComboBox` สำหรับ Select · Combobox = TextEdit + `Popup`/`Area` + list · Segmented/Chip = custom (`selectable_label` เป็นฐานได้)
