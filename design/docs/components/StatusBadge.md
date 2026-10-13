# StatusBadge
ป้ายสถานะสั้น ๆ และ StatusLine ที่แสดง 3 แกน (Order · Payment · Fulfillment) แยกกัน · **Ready**

**Variants** neutral · success · warning · danger · info · accent (พร้อมเสิร์ฟ / ไฮไลต์) · outline (ป้ายกำกับเช่น “Future”, “Proposed”)
**กติกา** ข้อความสั้น 1–3 คำเสมอ (ห้ามมีแค่สี) · icon เสริมได้ · StatusLine เรียง Order → Payment → Fulfillment เสมอ แสดงสูงสุด 3 ป้าย · ห้ามใช้ Badge เป็นปุ่ม
**Mapping ความหมาย** ปกติ/เสร็จ = success · ต้องทำอะไรบางอย่าง = warning · ล้มเหลว/ยกเลิก/หมด = danger · กำลังดำเนินการ = info · ยังไม่เริ่ม/ไม่มีผล = neutral
**ไม่ควรใช้** สีของสถานะแทนหมวดหมู่สินค้า
**Count** ตัวเลขบนพื้น inverse (จำนวนในตะกร้า, บิลพัก) · **Dot** ใช้คู่ข้อความใน StatusPill เท่านั้น
**egui** custom: `Frame` + `RichText` ขนาด 12 600 · ความสูง 22 · radius xs
