# Multi-device proposal (not built)

**สรุป:** ตอนนี้ POS Heart ใช้ได้เครื่องเดียว (ไฟล์ SQLite เดียว และ worker thread เดียวเป็นเจ้าของ) เจ้าของร้านเลือกเลื่อนเรื่องหลายเครื่องออกไปก่อน เอกสารนี้เป็นข้อเสนอไว้ตัดสินใจภายหลัง ยังไม่มีอะไรสร้างจริง ทางเลือก A: เครื่องหลักเก็บฐานข้อมูลและเปิดบริการในวง LAN ให้แท็บเล็ตสั่งอาหาร เครื่องขายที่สอง และจอครัวส่งคำสั่งมา กฎธุรกรรมเดิมใช้ได้ทั้งหมด งานขนาดกลางถึงใหญ่ เหมาะกับร้านเดียว ทางเลือก B: แต่ละเครื่องมีฐานข้อมูลของตัวเองแล้วซิงก์ผ่านเซิร์ฟเวอร์ งานใหญ่ เหมาะเมื่อมีหลายสาขาเท่านั้น **แนะนำ A ก่อน** เมื่อมีร้านจริงต้องการมากกว่าหนึ่งเครื่อง

Status: proposal for a future decision. Nothing here is implemented; the app stays single-register and offline.

## 1. Why this is hard here

- **One file, one writer.** A single worker thread owns the only SQLite connection (`apps/pos-desktop/src/worker.rs`). Every write is one IMMEDIATE transaction on that connection. SQLite over a network share is not safe, so "open the same file from two machines" is not an option.
- **Invariants assume one writer:**
  - receipt numbers, tax-invoice numbers and kitchen-ticket numbers are local AUTOINCREMENT sequences;
  - stock is checked and deducted inside the checkout transaction (no reservations, no negative stock);
  - one active shift (partial unique index) and one drawer;
  - one open bill per table (`hold()` checks inside its transaction);
  - retry identity (cart ID / request UUID) is looked up before the shift gate, assuming the same database saw the first attempt;
  - receipts, voids, refunds, closed shifts and kitchen tickets are immutable snapshots written once.
- **"No network by default."** AGENTS.md allows only a user-configured LAN printer. Any multi-device design adds a listening service, so it must be an explicit opt-in with its own security story.

## 2. Option A — main register + LAN clients

One machine (the main register) owns the database and runs a small local service that exposes the **same typed commands the worker already handles**. Clients (ordering tablets, a second register, a kitchen screen on another machine) send commands and render the results. All transactions still run on the main machine, so every invariant above holds unchanged.

Needs:
- **Pairing:** a one-time code shown on the main register; each device then gets its own token (revocable in Settings). Commands are still attributed to the signed-in staff member.
- **LAN-only binding:** listen on the local network only; TLS with pairing-based trust (pin the main register's key at pairing) rather than a public CA.
- **Discovery:** mDNS, with a typed IP address as fallback.
- **Offline behaviour:** clients cannot sell when the main machine is down; they show a clear "main register unreachable" state. No local queueing of payments.
- **Protocol versioning:** a version handshake; mismatched clients are refused with an update message.
- **Per-register drawers:** shifts gain `register_id`; one active shift per register instead of per shop; reports and drawer statements by register as well as shop totals.

Effort: medium–large. Good fit for one shop with a few devices.

## 3. Option B — offline-first sync between devices or branches

Each device keeps its own database and syncs through a server.

Needs:
- globally unique receipt numbers (device/branch prefix + local sequence), and the same for tax invoices (which have legal numbering rules);
- stock conflict rules: allow negative stock then reconcile, or reserve stock per device;
- a central authority for catalog, prices, roles and settings;
- per-device shifts and drawers;
- idempotent, ordered event replication with conflict handling for every record type;
- a server (cloud or on-premises), accounts, authentication, backups and monitoring.

Effort: large. Only worth it for multiple branches.

## 4. Recommendation

Do **A first**, and only when a real shop needs more than one device. Consider **B** only for multiple branches; A's protocol work carries over.

Concrete first steps for A:
1. Migration adding `register_id` to shifts (and to sales/ledger rows), with the existing register as the default.
2. Move the worker's command enum and results into a serializable protocol crate shared by the desktop and the service.
3. A local service on the main register behind an explicit opt-in setting (off by default; nothing listens otherwise).
4. Pairing UI: one-time code, device list, revoke.
5. A client mode in the app that talks to the service instead of opening SQLite.

## 5. Open questions for the owner

- How many devices, and which roles (second register, ordering tablets, kitchen display on another machine)?
- How reliable is the shop's Wi-Fi/LAN, and is a wired main register possible?
- Are ordering tablets at the tables actually needed, or is a second monitor for the kitchen (already built) enough?
- Are there plans for more branches, and on what timeline?
