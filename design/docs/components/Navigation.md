# Navigation
แอปมีโครงเดียวทุกหน้า: **TitleBar** (ปุ่มหน้าต่าง + แท็บ workspace + Ctrl K) → **Toolbar** ตามหน้า → **Panes** ที่ลอยบนพื้นกราไฟต์ → **StatusBar** · **Ready**

- **WorkspaceTabs** อยู่ในแถบชื่อโปรแกรม (unified title bar) · แท็บที่เลือก: พื้น `bg-chrome-hover` + เส้น `accent-bg` 2px ด้านล่าง · `Alt 1–7`
- **Toolbar** มีเฉพาะการกระทำของหน้านั้น จัดกลุ่มด้วยเส้นคั่น และ **ระบุขอบเขต** — กลุ่ม “ทั้งบิล” ใช้กับทั้งบิล; การกระทำของรายการเดียวอยู่ในแถบแก้ไขใต้บรรทัด ไม่อยู่ใน toolbar · ด้านขวาใช้ Segmented สลับมุมมอง (สินค้า / โต๊ะ)
- **Pane** = พื้นผิวทำงาน (`bg-surface`, `radius-lg`) หัวแผง 28px · เนื้อหาเลื่อนในตัว · ปุ่มยืนยันอยู่ใน **SaveBar** ตรึงล่างแผงเสมอ
- **StatusBar** ทุกช่อง **กดได้**: พนักงาน (สลับด้วย PIN), ออฟไลน์/sync (ดูรายการที่รอ), กะ/ลิ้นชัก (ไปหน้ากะ), เครื่องพิมพ์ (พิมพ์ทดสอบ), เสียงยืนยัน
- **CommandBar** (`Ctrl K`) ค้นได้ทั้งคำสั่ง หน้า และสินค้า · Enter = ผลลัพธ์แรก

**Touch** titlebar 48 / toolbar 58 / statusbar 32 · ซ่อนคำใบ้คีย์ลัด · ปุ่ม toolbar สูง 48
**egui** TitleBar: macOS `ViewportBuilder::with_fullsize_content_view(true).with_titlebar_shown(false).with_title_shown(false)` แล้ววาดแท็บเอง (เว้นที่ปุ่มหน้าต่าง ~70px); Windows: `with_decorations(false)` + ลากด้วย `ViewportCommand::StartDrag` และวาดปุ่มย่อ/ขยาย/ปิดเอง · Toolbar/StatusBar = `TopBottomPanel` · Pane = `Frame::new().fill(bg_surface).corner_radius(10)`
