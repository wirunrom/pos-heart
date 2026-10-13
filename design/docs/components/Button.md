# Button
ปุ่มสำหรับการกระทำ มี 4 ระดับความเด่น (primary · secondary · ghost · danger) และขนาด sm–xl รวมถึง IconButton และ ActionGroup · **Ready**

**ใช้เมื่อ** ผู้ใช้สั่งให้เกิดผลบางอย่าง · **ไม่ควรใช้** เพื่อนำทางไปหน้าอื่น (ใช้ nav/ลิงก์), เปิด/ปิดค่า (ใช้ Switch), หรือเลือกหนึ่งในหลายค่า (ใช้ Segmented)

**Anatomy** `[icon?] label [kbd hint?]` · ความสูง `control-*` · radius sm (compact) / md (touch) · label ใช้ `label` 600

| Variant | ใช้กับ | กติกา |
|---|---|---|
| `primary` | ขั้นต่อไปหลัก / การกระทำที่ทำให้เงินเคลื่อน | **1 ปุ่มต่อมุมมอง** · ข้อความเข้มบน saffron ทั้งสองธีม |
| secondary (ค่าเริ่มต้น) | ทางเลือกรอง | ขอบ `border-default` + label บอกชัดเจน |
| `ghost` | ยกเลิก, ปิด, action ในแถว | ห้ามใช้กับการกระทำหลัก |
| `danger` | ทำลายข้อมูลแบบย้อนกลับไม่ได้ (ใน Dialog) | · `danger-quiet` = ลบที่ undo ได้ |
| ขนาด `sm md lg xl` | `xl` ใช้กับปุ่มรับเงิน/ชำระเงิน · `sm` ใช้ในตาราง/BulkBar | |

**States** rest · hover · pressed · focus (ring น้ำเงิน) · disabled (0.45 + ไม่รับการกด) · loading (spinner คงความกว้างเดิม, กันกดซ้ำ — สำคัญกับการรับเงิน)
**Keyboard** `Enter`/`Space` กด · ปุ่มที่มีคีย์ลัดแสดง `kbd` บนเดสก์ท็อป (ซ่อนในโหมดสัมผัส แต่คีย์ลัดยังใช้ได้)
**A11y/touch** พื้นที่กด ≥ 44px ในโหมดสัมผัส · IconButton ต้องมี `aria-label` + tooltip และมีขอบ `border-control` (≥ 3:1)
**Content** ใช้คำกริยา + กรรม (“รับเงินสด”, “พักบิล”) · ห้ามตัด ellipsis ในปุ่ม — ปุ่มขยายตามข้อความ · ปุ่มเงินแสดงยอดเต็ม `รับเงินสด ฿1,323.50`
**ActionGroup** เรียง ghost → secondary → primary จากซ้ายไปขวา, primary อยู่ขวาสุด · บนจอแคบให้ขึ้นบรรทัดใหม่ ไม่ใช่ซ่อน
**egui** `Button::new(RichText)` + `min_size` สำหรับ secondary/ghost · primary/loading/kbd ทำเป็น custom widget (`allocate_exact_size` + painter)
