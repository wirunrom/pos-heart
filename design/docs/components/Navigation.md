# Navigation
Topbar (WorkspaceSwitcher · บริบทสาขา/เครื่อง · CommandBar · StatusPill), Tabs, SideNav และ Rail · **Ready** (CommandBar: Experimental)

- **WorkspaceSwitcher** ปุ่มแรกในแถบบน แสดง workspace ปัจจุบัน → เมนูจัดกลุ่ม “งานประจำวัน / จัดการร้าน” พร้อมคีย์ลัด `Alt 1–7` · ฟีเจอร์ที่ไม่ได้เปิดใช้จะไม่อยู่ในเมนู
- **CommandBar** (`Ctrl K`) ค้นหาได้ทั้ง สินค้า บิล คำสั่ง (“ปิดกะ”, “เปิดลิ้นชัก”) และหน้าตั้งค่า · ผลลัพธ์จัดกลุ่ม · ไม่ใช่ทางเดียวในการเข้าถึง
- **StatusPill** สถานะที่ต้องเห็นตลอด: กะ, ออฟไลน์, sync · จุดสี + ข้อความเสมอ · กดเพื่อดูรายละเอียด/action · offline ใช้จุดกลวงสีกลาง (ไม่ใช่ error)
- **Tabs** สลับมุมมองของเนื้อหาเดียวกัน (บิลปัจจุบัน / พักไว้) · active = ตัวหนา + เส้นใต้ · แสดงจำนวนด้วย Count
- **SideNav** หมวดหมู่/มุมมองที่บันทึกไว้ พร้อมจำนวน · หน้าจัดการและหมวดหมู่ในหน้าขาย
- **Rail** ทางเลือกสำหรับจอสัมผัส ≥ 1280 ที่อยากให้เห็น workspace ตลอด · icon + ป้ายเสมอ

**Keyboard** Tabs `←→` · เมนู `↑↓ Enter Esc` · `Ctrl K` เปิด command bar จากทุกที่
**Overflow** breadcrumb ตัด ellipsis ก่อน · CommandBar หดก่อน pill · บนจอ < 1100 pill กะย่อเป็น “กะ #12” · Tabs ที่เกินเลื่อนแนวนอนได้
**egui** `TopBottomPanel::top` + custom widgets · เมนูใช้ `menu_button`/`Popup` · Tabs custom (`selectable_label` + เส้นใต้ด้วย painter)
