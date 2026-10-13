# TextField
ช่องกรอกข้อความหนึ่งบรรทัด พร้อม MoneyField, QuantityStepper และ SearchField ที่ใช้โครงเดียวกัน · **Ready**

**Anatomy** label (อยู่เหนือช่องเสมอ) → control (`[affix] input [affix/kbd]`) → help หรือ error · ห้ามใช้ placeholder แทน label

| ชนิด | ต่างจากพื้นฐาน |
|---|---|
| **MoneyField** | `font-num` ตัวหนา ชิดขวา, ขอบ 2px `fg-default` เมื่อเป็นช่องหลักของหน้า (รับเงิน) · รับเฉพาะตัวเลข · จัดรูปแบบ `1,500.00` ตอน blur · focus แล้วเลือกค่าเดิมทั้งหมด · เก็บค่าเป็นสตางค์ (integer) |
| **QuantityStepper** | `− [n] +` · พิมพ์ตัวเลขตรงกลางได้ · รองรับทศนิยมสูงสุด 3 ตำแหน่งสำหรับสินค้าชั่งน้ำหนัก · ลดถึง 0 = เสนอให้ลบบรรทัด (undo ได้) ไม่ใช่ลบทันที |
| **SearchField** | icon ค้นหา + คีย์ลัด (`F2`, `/`) · `Esc` ล้างคำค้น · รับ input จากเครื่องสแกน barcode |

**States** rest · hover · focus (ring) · invalid (ขอบ+เส้นซ้อน `danger-fg` + ข้อความพร้อม icon) · readonly (พื้น sunken ขอบเส้นประ) · disabled
**Validation** ตรวจตอน blur และตอนบันทึก (ยกเว้นเงินที่คำนวณเงินทอนแบบ live) · ข้อความบอกวิธีแก้ ไม่ใช่แค่บอกว่าผิด
**A11y** label ผูกกับ input · error ผูกด้วย `aria-describedby` และประกาศเมื่อปรากฏ · ช่องไม่บังคับเขียน “(ไม่บังคับ)”
**Overflow** ข้อความยาวเลื่อนในช่อง · ช่องชื่อสินค้าในฟอร์มแก้ไขใช้แบบหลายบรรทัด (auto-grow ≤ 3 บรรทัด)
**egui** `TextEdit::singleline` + `Frame` ของเราเอง · MoneyField = TextEdit ที่กรองตัวอักษร + format เมื่อ `lost_focus()` · Stepper = custom
