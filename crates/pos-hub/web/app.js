// POS Heart ordering app for iPad: tables → menu → bill → send to kitchen.
// Talks to the main register's hub over HTTPS (same origin): /api/pair once,
// then /api/state, /api/call and the /api/changes long poll. No payment here.
"use strict";

const $ = (id) => document.getElementById(id);
const KEY = "posheart.device";
let device = load();
let state = null;       // last /api/state
let total = 0;          // change counter the state belongs to
let open = null;        // { table, cart, dirty } while a bill is open
let category = null;
let search = "";
let busy = false;

function load() {
  try { return JSON.parse(localStorage.getItem(KEY) || "null"); } catch { return null; }
}
function save(d) {
  device = d;
  try { d ? localStorage.setItem(KEY, JSON.stringify(d)) : localStorage.removeItem(KEY); } catch { /* private mode */ }
}
function uuid() {
  if (crypto.randomUUID) return crypto.randomUUID();
  const b = crypto.getRandomValues(new Uint8Array(16));
  b[6] = (b[6] & 15) | 64; b[8] = (b[8] & 63) | 128;
  const h = [...b].map((x) => x.toString(16).padStart(2, "0")).join("");
  return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
}
function el(tag, attrs = {}, ...kids) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "class") e.className = v;
    else if (k.startsWith("on")) e.addEventListener(k.slice(2), v);
    else if (v === true) e.setAttribute(k, "");
    else if (v !== false && v != null) e.setAttribute(k, v);
  }
  for (const k of kids.flat()) if (k != null && k !== false) e.append(k instanceof Node ? k : String(k));
  return e;
}
// Satang → "1,234.50" (U+2212 minus).
function money(s) {
  const neg = s < 0; s = Math.abs(s);
  const baht = Math.floor(s / 100).toLocaleString("en-US");
  return (neg ? "−" : "") + baht + "." + String(s % 100).padStart(2, "0");
}
let toastTimer = 0;
function toast(text, bad = false) {
  const t = $("toast");
  t.textContent = text; t.className = bad ? "toast bad" : "toast"; t.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { t.hidden = true; }, bad ? 5000 : 2200);
}
const ERRORS = {
  TableBusy: "โต๊ะนี้มีบิลเปิดอยู่แล้ว",
  TableNotFound: "ไม่พบโต๊ะนี้",
  AlreadyPaid: "บิลนี้ชำระเงินแล้ว",
  InsufficientStock: "สินค้าในสต็อกไม่พอ",
  ProductUnavailable: "สินค้านี้ไม่มีแล้ว",
  NotForSale: "สินค้านี้ปิดการขาย",
  SignInRequired: "ต้องเข้าสู่ระบบพนักงานก่อน",
  Forbidden: "บทบาทนี้ไม่มีสิทธิ์ทำรายการนี้",
  WrongPin: "PIN ไม่ถูกต้อง",
  PinLocked: "ใส่ PIN ผิดหลายครั้ง ลองใหม่ภายหลัง",
  NotPaired: "iPad นี้ถูกยกเลิกการจับคู่ จับคู่ใหม่อีกครั้ง",
  WrongCode: "รหัสไม่ถูกต้องหรือหมดอายุ ขอรหัสใหม่ที่เครื่องหลัก",
  Stopped: "เครื่องหลักหยุดทำงาน",
  Domain: "ข้อมูลบิลไม่ถูกต้อง",
};
function describe(err) {
  return ERRORS[err.kind] || err.message || "เกิดข้อผิดพลาด";
}

// ---- HTTP ----------------------------------------------------------------
class ApiError extends Error {
  constructor(error, status) { super(error.message); this.error = error; this.status = status; }
}
async function api(method, path, body) {
  const headers = {};
  if (device) headers.Authorization = `Bearer ${device.device}.${device.token}`;
  if (body !== undefined) headers["Content-Type"] = "application/json";
  const res = await fetch(path, { method, headers, body: body === undefined ? undefined : JSON.stringify(body), cache: "no-store" });
  const data = await res.json().catch(() => ({}));
  if (res.status === 401) {
    save(null); state = null; open = null; render();
    throw new ApiError(data.error || { kind: "NotPaired", message: "" }, 401);
  }
  if (data.error) throw new ApiError(data.error, res.status);
  if (!res.ok) throw new ApiError({ kind: "Http", message: `HTTP ${res.status}` }, res.status);
  return data;
}
// One request with a stable call ID; a dropped connection is retried once
// (writes are retry-safe by their cart/request IDs).
async function call(request) {
  const id = uuid();
  try {
    return (await api("POST", "/api/call", { id, request })).ok;
  } catch (e) {
    if (e instanceof ApiError) throw e;
    await new Promise((r) => setTimeout(r, 800));
    return (await api("POST", "/api/call", { id, request })).ok;
  }
}
// Runs a write; asks for a manager's PIN when the role needs approval.
async function write(request) {
  try {
    return await call(request);
  } catch (e) {
    if (e instanceof ApiError && e.error.kind === "ApprovalRequired" && e.error.permission) {
      const approval = await pinSheet("ต้องให้ผู้มีสิทธิ์อนุมัติ", true);
      if (!approval) return null;
      return await call({ Approved: { approver: approval.id, pin: approval.pin, permission: e.error.permission, inner: request } });
    }
    throw e;
  }
}
async function refresh() {
  const s = await api("GET", "/api/state");
  state = s; total = s.total;
  if (open && !open.dirty) {
    const held = state.held.find((c) => c.id === open.cart.id) || state.held.find((c) => c.table === open.table.id);
    if (held) open.cart = held;
  }
  setStatus(true);
  render();
}
function setStatus(ok) {
  const s = $("status");
  s.textContent = ok ? "" : "ขาดการเชื่อมต่อ…";
  s.className = ok ? "status" : "status off";
}
async function listen() {
  for (;;) {
    if (!device) { await new Promise((r) => setTimeout(r, 1000)); continue; }
    try {
      const c = await api("GET", `/api/changes?since=${total}`);
      setStatus(true);
      if (c.total !== total) await refresh();
    } catch (e) {
      setStatus(false);
      await new Promise((r) => setTimeout(r, 3000));
      if (device) refresh().catch(() => {});
    }
  }
}

// ---- Views ---------------------------------------------------------------
function render() {
  const view = $("view");
  view.replaceChildren();
  $("back").hidden = !open;
  const who = $("who");
  who.hidden = !(state && state.staff_enabled);
  if (state && state.signed_in) who.textContent = `${state.signed_in.name} · ออก`;
  else who.textContent = "เข้าสู่ระบบ";
  if (!device) { $("title").textContent = "จับคู่ iPad"; view.append(pairView()); return; }
  if (!state) { $("title").textContent = "POS Heart"; view.append(el("div", { class: "center" }, "กำลังโหลด…")); return; }
  if (open) { $("title").textContent = open.cart.label || open.table.name; view.append(...orderView()); return; }
  $("title").textContent = state.shop || "POS Heart";
  view.append(floorView());
}

function pairView() {
  const code = el("input", { class: "code", inputmode: "numeric", autocomplete: "one-time-code", maxlength: "6", placeholder: "000000", "aria-label": "รหัสจับคู่ 6 หลัก" });
  const name = el("input", { value: "iPad", maxlength: "40", "aria-label": "ชื่อเครื่อง" });
  const go = el("button", { class: "primary", type: "submit" }, "จับคู่");
  const form = el("form", { class: "pane center", onsubmit: async (ev) => {
    ev.preventDefault();
    go.disabled = true;
    try {
      const paired = await api("POST", "/api/pair", { code: code.value.trim(), name: name.value.trim() || "iPad" });
      save({ device: paired.device, token: paired.token });
      toast("จับคู่แล้ว");
      await refresh();
    } catch (e) {
      toast(e instanceof ApiError ? describe(e.error) : "เชื่อมต่อเครื่องหลักไม่ได้", true);
      go.disabled = false;
    }
  } },
    el("h2", {}, "จับคู่กับเครื่องหลัก"),
    el("p", { class: "muted" }, "ที่เครื่องหลัก เปิด ตั้งค่า → อุปกรณ์ในร้าน → จับคู่อุปกรณ์ แล้วใส่รหัส 6 หลักที่แสดง"),
    el("label", { class: "field" }, el("span", {}, "รหัสจับคู่"), code),
    el("label", { class: "field" }, el("span", {}, "ชื่อ iPad เครื่องนี้"), name),
    go);
  form.style.display = "block";
  setTimeout(() => code.focus(), 50);
  return form;
}

function billOf(table) {
  return state.held.find((c) => c.table === table.id);
}
function units(cart) {
  return cart.lines.reduce((n, l) => n + l.quantity, 0);
}
function floorView() {
  const pane = el("section", { class: "pane floor" }, el("header", {}, "ผังโต๊ะ"));
  const list = el("div", { class: "scroll" });
  if (!state.tables.length) list.append(el("p", { class: "empty" }, "ยังไม่มีโต๊ะ เพิ่มโต๊ะได้ที่เครื่องหลัก: ตั้งค่า → โต๊ะ"));
  const zones = [...new Set(state.tables.map((t) => t.zone))];
  for (const zone of zones) {
    if (zone) list.append(el("div", { class: "zone" }, zone));
    const grid = el("div", { class: "tables" });
    for (const t of state.tables.filter((x) => x.zone === zone)) {
      const bill = billOf(t);
      grid.append(el("button", { class: bill ? "table busy" : "table", onclick: () => openTable(t) },
        el("b", {}, t.name),
        el("small", { class: "muted" }, bill ? `${units(bill)} รายการ · ${money(subtotal(bill))}` : `ว่าง · ${t.seats} ที่นั่ง`)));
    }
    list.append(grid);
  }
  pane.append(list);
  return pane;
}
function openTable(table) {
  const held = billOf(table);
  const cart = held ? structuredClone(held) : { id: uuid(), label: table.name, lines: [], service_charge: true, table: table.id, guests: 0 };
  open = { table, cart, dirty: false };
  category = null; search = "";
  render();
}

function product(id) {
  return state.products.find((p) => p.id === id);
}
function unitPrice(line) {
  const p = product(line.product_id);
  if (!p) return 0;
  let price = p.price;
  for (const o of line.options || []) {
    const g = p.options.find((x) => x.id === o.group_id);
    const c = g && g.choices.find((x) => x.id === o.choice_id);
    if (c) price += c.price;
  }
  return price;
}
function subtotal(cart) {
  return cart.lines.reduce((s, l) => s + unitPrice(l) * l.quantity, 0);
}
function sentOf(line) {
  const same = (a, b) => JSON.stringify(a || []) === JSON.stringify(b || []);
  const item = (open.cart.sent || []).find((s) => s.product_id === line.product_id && same(s.options, line.options) && (s.note || "") === (line.note || ""));
  return item ? item.quantity : 0;
}

function orderView() {
  const menu = el("section", { class: "pane menu" });
  const cats = el("div", { class: "cats" });
  const chip = (name, label) => el("button", { class: category === name ? "chip on" : "chip", onclick: () => { category = name; render(); } }, label);
  cats.append(chip(null, "ทั้งหมด"));
  for (const c of state.categories) if (state.products.some((p) => p.category === c)) cats.append(chip(c, c));
  const find = el("input", { class: "search", type: "search", placeholder: "ค้นหาเมนู", value: search, "aria-label": "ค้นหาเมนู" });
  const items = el("div", { class: "items" });
  const fill = () => {
    items.replaceChildren();
    const q = search.trim().toLowerCase();
    const list = state.products.filter((p) => (category == null || p.category === category) && (!q || p.name.toLowerCase().includes(q)));
    if (!list.length) items.append(el("p", { class: "empty" }, "ไม่พบเมนู"));
    for (const p of list) {
      const out = p.stock != null && p.stock <= 0;
      items.append(el("button", { class: out ? "item out" : "item", disabled: out, "aria-label": `${p.name} ${money(p.price)}`, onclick: () => add(p) },
        el("span", { class: "name" }, p.name),
        el("span", { class: "money" }, out ? "หมด" : money(p.price))));
    }
  };
  find.addEventListener("input", () => { search = find.value; fill(); });
  fill();
  menu.append(cats, find, el("div", { class: "scroll" }, items));

  const bill = el("section", { class: "pane bill" });
  const cart = open.cart;
  bill.append(el("header", {}, el("span", { class: "grow" }, `บิล ${open.table.name}`),
    el("button", { class: "chip", onclick: guests }, cart.guests ? `${cart.guests} ท่าน` : "จำนวนลูกค้า")));
  const lines = el("ul", { class: "lines" });
  cart.lines.forEach((line, i) => {
    const p = product(line.product_id);
    const opts = (line.options || []).map((o) => {
      const g = p && p.options.find((x) => x.id === o.group_id);
      const c = g && g.choices.find((x) => x.id === o.choice_id);
      return c ? c.name : "";
    }).filter(Boolean).join(", ");
    const sent = sentOf(line);
    lines.append(el("li", { class: "line" },
      el("span", {}, p ? p.name : "สินค้าที่ลบแล้ว"),
      el("span", { class: "money" }, money(unitPrice(line) * line.quantity)),
      opts && el("span", { class: "opts" }, opts),
      line.note && el("span", { class: "note" }, `หมายเหตุ: ${line.note}`),
      sent > 0 && el("span", { class: "sent" }, `ส่งครัวแล้ว ${Math.min(sent, line.quantity)}`),
      el("div", { class: "qty" },
        el("button", { "aria-label": "ลด", onclick: () => change(i, -1) }, "−"),
        el("span", { class: "n" }, line.quantity),
        el("button", { "aria-label": "เพิ่ม", onclick: () => change(i, 1) }, "+"),
        el("button", { class: "noteb", onclick: () => note(i) }, "หมายเหตุ"))));
  });
  const unsent = cart.lines.some((l) => sentOf(l) !== l.quantity) || (cart.sent || []).length > 0 && open.dirty;
  bill.append(el("div", { class: "scroll" }, cart.lines.length ? lines : el("p", { class: "empty" }, "แตะเมนูเพื่อเพิ่มรายการ")),
    el("div", { class: "foot" },
      el("div", { class: "sum" }, el("span", {}, "ยอดรวม (ก่อนส่วนลด/ค่าบริการ)"), el("span", { class: "money" }, money(subtotal(cart)))),
      el("div", { class: "row" },
        el("button", { class: "secondary", disabled: busy || !open.dirty, onclick: hold }, "บันทึก"),
        el("button", { class: "primary", disabled: busy || !unsent, onclick: send }, busy ? "กำลังส่ง…" : "ส่งครัว"))));
  return [menu, bill];
}

function edited() { open.dirty = true; render(); }
function add(p) {
  if (p.options && p.options.length) { optionSheet(p); return; }
  addLine(p, []);
}
function addLine(p, options) {
  const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
  const line = open.cart.lines.find((l) => l.product_id === p.id && same(l.options || [], options) && !l.note);
  if (line) line.quantity += 1;
  else open.cart.lines.push({ product_id: p.id, quantity: 1, note: "", options });
  edited();
}
function change(i, d) {
  const line = open.cart.lines[i];
  line.quantity += d;
  if (line.quantity <= 0) open.cart.lines.splice(i, 1);
  edited();
}
function note(i) {
  const line = open.cart.lines[i];
  const text = prompt("หมายเหตุถึงครัว", line.note || "");
  if (text === null) return;
  line.note = text.trim().slice(0, 100);
  edited();
}
function guests() {
  const n = parseInt(prompt("จำนวนลูกค้า", open.cart.guests || "") || "", 10);
  if (!Number.isFinite(n) || n < 0 || n > 999) return;
  open.cart.guests = n;
  edited();
}
async function send() {
  if (busy) return;
  busy = true; render();
  try {
    const reply = await write({ SendKitchen: open.cart });
    if (reply) {
      toast("ส่งครัวแล้ว");
      open = null;
      await refresh();
    }
  } catch (e) {
    toast(e instanceof ApiError ? describe(e.error) : "ส่งไม่สำเร็จ ตรวจการเชื่อมต่อแล้วลองอีกครั้ง", true);
  } finally {
    busy = false; render();
  }
}
async function hold() {
  if (busy) return;
  busy = true; render();
  try {
    const reply = open.cart.lines.length ? await write({ Hold: open.cart }) : await write({ RemoveHeld: open.cart.id });
    if (reply) {
      toast("บันทึกบิลแล้ว");
      open = null;
      await refresh();
    }
  } catch (e) {
    toast(e instanceof ApiError ? describe(e.error) : "บันทึกไม่สำเร็จ", true);
  } finally {
    busy = false; render();
  }
}

// ---- Sheets --------------------------------------------------------------
function sheet(card) {
  const s = $("sheet");
  s.replaceChildren(card); s.hidden = false;
}
function closeSheet() { $("sheet").hidden = true; $("sheet").replaceChildren(); }

function optionSheet(p) {
  const picked = new Map(p.options.map((g) => [g.id, new Set()]));
  const ok = el("button", { class: "primary" });
  const body = el("div", { class: "body" });
  const update = () => {
    const ready = p.options.every((g) => !g.required || picked.get(g.id).size > 0);
    let price = p.price;
    for (const g of p.options) for (const c of g.choices) if (picked.get(g.id).has(c.id)) price += c.price;
    ok.disabled = !ready;
    ok.textContent = `เพิ่ม · ${money(price)}`;
  };
  for (const g of p.options) {
    const row = el("div", { class: "choices" });
    for (const c of g.choices) {
      const b = el("button", { class: "choice", onclick: () => {
        const set = picked.get(g.id);
        if (set.has(c.id)) set.delete(c.id);
        else { if (!g.multiple) set.clear(); set.add(c.id); }
        for (const x of row.children) x.classList.toggle("on", set.has(x.dataset.id));
        update();
      } }, c.name, c.price ? ` +${money(c.price)}` : "");
      b.dataset.id = c.id;
      row.append(b);
    }
    body.append(el("div", { class: "group" }, el("h3", {}, g.name, " ", el("small", {}, g.required ? (g.multiple ? "เลือกอย่างน้อย 1" : "เลือก 1") : (g.multiple ? "เลือกได้หลายอย่าง" : "ไม่บังคับ"))), row));
  }
  ok.addEventListener("click", () => {
    const options = [];
    for (const g of p.options) for (const c of g.choices) if (picked.get(g.id).has(c.id)) options.push({ group_id: g.id, choice_id: c.id });
    closeSheet();
    addLine(p, options);
  });
  update();
  sheet(el("div", { class: "card" }, el("header", {}, p.name), body,
    el("div", { class: "actions" }, el("button", { class: "secondary", onclick: closeSheet }, "ยกเลิก"), ok)));
}

// Picks a staff member and their PIN. Resolves { id, pin } or null.
function pinSheet(title, approval) {
  return new Promise((resolve) => {
    let who = null; let pin = "";
    const dots = el("div", { class: "dots" });
    const body = el("div", { class: "body" });
    const ok = el("button", { class: "primary", disabled: true }, approval ? "อนุมัติ" : "เข้าสู่ระบบ");
    const finish = (v) => { closeSheet(); resolve(v); };
    const draw = () => {
      dots.textContent = "•".repeat(pin.length) || " ";
      ok.disabled = !who || pin.length < 4;
    };
    const people = el("div", { class: "staff" });
    for (const s of state.staff) {
      const b = el("button", { class: "choice", onclick: () => { who = s.id; for (const x of people.children) x.classList.toggle("on", x === b); draw(); } }, s.name);
      people.append(b);
    }
    const pad = el("div", { class: "pins" });
    for (const k of ["1", "2", "3", "4", "5", "6", "7", "8", "9", "ล้าง", "0", "⌫"]) {
      pad.append(el("button", { "aria-label": k === "⌫" ? "ลบ" : k, onclick: () => {
        if (k === "ล้าง") pin = ""; else if (k === "⌫") pin = pin.slice(0, -1); else if (pin.length < 8) pin += k;
        draw();
      } }, k));
    }
    body.append(people, dots, pad);
    ok.addEventListener("click", () => finish({ id: who, pin }));
    draw();
    sheet(el("div", { class: "card" }, el("header", {}, title), body,
      el("div", { class: "actions" }, el("button", { class: "secondary", onclick: () => finish(null) }, "ยกเลิก"), ok)));
  });
}

$("who").addEventListener("click", async () => {
  try {
    if (state.signed_in) {
      await call("SignOut");
      toast("ออกจากระบบแล้ว");
    } else {
      const v = await pinSheet("เข้าสู่ระบบพนักงาน", false);
      if (!v) return;
      await call({ SignIn: [v.id, v.pin] });
      toast("เข้าสู่ระบบแล้ว");
    }
    await refresh();
  } catch (e) {
    toast(e instanceof ApiError ? describe(e.error) : "เชื่อมต่อไม่ได้", true);
  }
});
$("back").addEventListener("click", () => {
  if (open && open.dirty && !confirm("ยังไม่ได้บันทึกหรือส่งครัว ทิ้งการแก้ไข?")) return;
  open = null; render();
});

render();
if (device) refresh().catch(() => setStatus(false));
listen();
