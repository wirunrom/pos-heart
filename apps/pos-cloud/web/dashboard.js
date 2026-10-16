// POS Heart owner dashboard: sign in, see sales, edit the menu, link the
// shop's main register. Talks to /api/v1 on the same origin with a bearer
// session token. Amounts are satang.
"use strict";

const $ = (id) => document.getElementById(id);
const KEY = "posheart.cloud.session";
const SHOP = "posheart.cloud.shop";
let token = read(KEY);
let shops = [];
let shopId = read(SHOP);
let tab = "sales";
let period = "today";

function read(k) { try { return localStorage.getItem(k); } catch { return null; } }
function write(k, v) { try { v == null ? localStorage.removeItem(k) : localStorage.setItem(k, v); } catch { /* private mode */ } }
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
function money(s) {
  const neg = s < 0; s = Math.abs(Math.round(s));
  return (neg ? "−" : "") + Math.floor(s / 100).toLocaleString("en-US") + "." + String(s % 100).padStart(2, "0");
}
function when(ts) {
  if (!ts) return "—";
  return new Date(ts * 1000).toLocaleString("th-TH", { dateStyle: "medium", timeStyle: "short" });
}
let toastTimer = 0;
function toast(text, bad = false) {
  const t = $("toast");
  t.textContent = text; t.className = bad ? "toast bad" : "toast"; t.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { t.hidden = true; }, bad ? 5000 : 2500);
}
const ERRORS = {
  WrongPassword: "อีเมลหรือรหัสผ่านไม่ถูกต้อง",
  EmailTaken: "อีเมลนี้มีบัญชีแล้ว ลองเข้าสู่ระบบ",
  Unauthorized: "หมดเวลาเข้าสู่ระบบ กรุณาเข้าใหม่",
};
async function api(method, path, body) {
  const headers = {};
  if (token) headers.Authorization = `Bearer ${token}`;
  if (body !== undefined) headers["Content-Type"] = "application/json";
  const res = await fetch(`/api/v1${path}`, { method, headers, body: body === undefined ? undefined : JSON.stringify(body) });
  const data = res.status === 204 ? null : await res.json().catch(() => null);
  if (res.status === 401 && token) {
    token = null; write(KEY, null); render();
  }
  if (!res.ok) {
    const err = data && data.error ? data.error : { kind: "Http", message: `HTTP ${res.status}` };
    throw new Error(ERRORS[err.kind] || err.message);
  }
  return data;
}
async function guard(fn) {
  try { await fn(); } catch (e) { toast(e.message || "เชื่อมต่อไม่ได้", true); }
}

// ---- Dates (Bangkok) ---------------------------------------------------
function bangkokToday() {
  return new Date(Date.now() + 7 * 3600e3).toISOString().slice(0, 10);
}
function addDays(date, n) {
  const d = new Date(`${date}T00:00:00Z`); d.setUTCDate(d.getUTCDate() + n);
  return d.toISOString().slice(0, 10);
}
function range() {
  const today = bangkokToday();
  switch (period) {
    case "yesterday": return [addDays(today, -1), addDays(today, -1)];
    case "7": return [addDays(today, -6), today];
    case "30": return [addDays(today, -29), today];
    case "month": return [today.slice(0, 8) + "01", today];
    default: return [today, today];
  }
}

// ---- Views -------------------------------------------------------------
let renders = 0;
async function render() {
  const view = $("view");
  const mine = ++renders;
  view.replaceChildren(el("p", { class: "muted" }, "กำลังโหลด…"));
  $("out").hidden = !token;
  $("tabs").hidden = true;
  $("shop").hidden = true;
  if (!token) { view.replaceChildren(authView()); return; }
  await guard(async () => { shops = await api("GET", "/shops"); });
  if (!token || mine !== renders) return;
  if (!shops.length) { view.replaceChildren(newShopView()); return; }
  if (!shops.some((s) => s.id === shopId)) shopId = shops[0].id;
  write(SHOP, shopId);
  const sel = $("shop");
  sel.replaceChildren(...shops.map((s) => el("option", { value: s.id, selected: s.id === shopId }, s.name)));
  sel.hidden = shops.length < 2;
  $("title").textContent = shops.find((s) => s.id === shopId).name;
  $("tabs").hidden = false;
  for (const b of $("tabs").children) b.classList.toggle("on", b.dataset.tab === tab);
  let content = [];
  await guard(async () => {
    content = tab === "sales" ? await salesView() : tab === "menu" ? await menuView() : await shopView();
  });
  // A newer render (another tab or period) wins.
  if (mine === renders) view.replaceChildren(...content);
}

function authView() {
  let signUp = false;
  const email = el("input", { type: "email", autocomplete: "username", required: true });
  const password = el("input", { type: "password", autocomplete: "current-password", minlength: "10", required: true });
  const go = el("button", { class: "primary", type: "submit" }, "เข้าสู่ระบบ");
  const head = el("h2", {}, "เข้าสู่ระบบเจ้าของร้าน");
  const swap = el("button", { class: "link", type: "button", onclick: () => {
    signUp = !signUp;
    head.textContent = signUp ? "สร้างบัญชีเจ้าของร้าน" : "เข้าสู่ระบบเจ้าของร้าน";
    go.textContent = signUp ? "สร้างบัญชี" : "เข้าสู่ระบบ";
    swap.textContent = signUp ? "มีบัญชีแล้ว? เข้าสู่ระบบ" : "ยังไม่มีบัญชี? สร้างบัญชี";
    password.autocomplete = signUp ? "new-password" : "current-password";
  } }, "ยังไม่มีบัญชี? สร้างบัญชี");
  const form = el("form", { class: "pane center", onsubmit: (ev) => {
    ev.preventDefault();
    guard(async () => {
      const r = await api("POST", signUp ? "/accounts" : "/sessions", { email: email.value, password: password.value });
      token = r.token; write(KEY, token);
      await render();
    });
  } },
    head,
    el("label", { class: "field" }, el("span", {}, "อีเมล"), email),
    el("label", { class: "field" }, el("span", {}, "รหัสผ่าน (อย่างน้อย 10 ตัว)"), password),
    go, swap);
  return form;
}

function newShopView() {
  const name = el("input", { maxlength: "80", required: true, placeholder: "เช่น ร้านกาแฟหน้าปากซอย" });
  return el("form", { class: "pane center", onsubmit: (ev) => {
    ev.preventDefault();
    guard(async () => {
      const shop = await api("POST", "/shops", { name: name.value });
      shopId = shop.id; tab = "shop";
      await render();
    });
  } },
    el("h2", {}, "เพิ่มร้านของคุณ"),
    el("p", { class: "muted" }, "ตั้งชื่อร้านบนคลาวด์ แล้วเชื่อมเครื่องหลักในร้านด้วยรหัสในขั้นถัดไป"),
    el("label", { class: "field" }, el("span", {}, "ชื่อร้าน"), name),
    el("button", { class: "primary", type: "submit" }, "เพิ่มร้าน"));
}

async function salesView() {
  const [from, to] = range();
  const s = await api("GET", `/shops/${shopId}/summary?from=${from}&to=${to}`);
  const chips = el("div", { class: "periods" }, ...[
    ["today", "วันนี้"], ["yesterday", "เมื่อวาน"], ["7", "7 วัน"], ["30", "30 วัน"], ["month", "เดือนนี้"],
  ].map(([k, label]) => el("button", { class: period === k ? "chip on" : "chip", onclick: () => { period = k; render(); } }, label)));
  const tile = (label, value) => el("div", { class: "tile" }, el("div", { class: "label" }, label), el("div", { class: "value" }, value));
  const head = el("section", { class: "pane" }, chips,
    el("p", { class: "muted" }, `ข้อมูลจากร้านล่าสุด ${when(s.uploaded_at)}`),
    el("div", { class: "tiles" },
      tile("ยอดสุทธิ", money(s.net)),
      tile("ยอดขาย", money(s.sales)),
      tile("คืนเงิน", money(s.refunds)),
      tile("จำนวนบิล", s.sale_count.toLocaleString("en-US"))));
  const single = s.days.length === 1;
  const series = single ? s.hourly : s.days.map((d) => d.sales);
  const max = Math.max(1, ...series);
  const chart = el("section", { class: "pane" }, el("h2", {}, single ? "ยอดขายรายชั่วโมง" : "ยอดขายรายวัน"),
    el("div", { class: "bars", role: "img", "aria-label": single ? "ยอดขายรายชั่วโมง" : "ยอดขายรายวัน" },
      ...series.map((v, i) => {
        const bar = el("span", { title: `${single ? `${i}:00` : s.days[i].date} · ${money(v)}` });
        bar.style.height = `${Math.round((v / max) * 100)}%`;
        return bar;
      })));
  const names = { cash: "เงินสด", transfer: "โอน", card: "บัตร" };
  const methods = el("section", { class: "pane" }, el("h2", {}, "ตามวิธีชำระ"),
    el("div", { class: "rows" }, ...Object.entries(s.methods).map(([k, m]) =>
      el("div", { class: "row" }, el("span", {}, names[k] || k), el("span", { class: "money" }, money(m.sales - m.refunds)),
        m.refunds ? el("span", { class: "sub" }, `คืนเงิน ${money(m.refunds)}`) : null))),
    Object.keys(s.methods).length ? null : el("p", { class: "muted" }, "ยังไม่มียอดขายในช่วงนี้"));
  const top = el("section", { class: "pane" }, el("h2", {}, "สินค้าขายดี"),
    el("div", { class: "rows" }, ...s.top_products.map((p) =>
      el("div", { class: "row" }, el("span", {}, p.name), el("span", { class: "money" }, money(p.amount)),
        el("span", { class: "sub" }, `${p.quantity.toLocaleString("en-US")} ชิ้น`)))),
    s.top_products.length ? null : el("p", { class: "muted" }, "ยังไม่มียอดขายในช่วงนี้"));
  return [head, chart, el("div", { class: "split" }, methods, top)];
}

async function menuView() {
  const [{ products }, changes] = await Promise.all([
    api("GET", `/shops/${shopId}/products`),
    api("GET", `/shops/${shopId}/changes`),
  ]);
  const waiting = changes.filter((c) => !c.applied_at);
  const pane = el("section", { class: "pane" }, el("h2", {}, "เมนูและราคา"),
    el("p", { class: "muted" }, "แก้ชื่อหรือราคาแล้วกดบันทึก เครื่องหลักในร้านจะรับไปใช้เมื่อซิงก์ครั้งถัดไป (ประมาณ 30 วินาทีถ้าออนไลน์)"));
  if (waiting.length) pane.append(el("p", {}, el("span", { class: "badge" }, `รอร้านรับ ${waiting.length} รายการ`)));
  let category = null;
  for (const p of products) {
    if (p.category !== category) {
      category = p.category;
      pane.append(el("h2", {}, category || "ไม่มีหมวด"));
    }
    const name = el("input", { value: p.name, maxlength: "80", "aria-label": "ชื่อสินค้า" });
    const price = el("input", { class: "money", value: (p.price / 100).toFixed(2), inputmode: "decimal", "aria-label": "ราคา" });
    const onSale = el("input", { type: "checkbox", checked: p.on_sale, "aria-label": "เปิดขาย" });
    const save = el("button", { class: "secondary", onclick: () => guard(async () => {
      const patch = {};
      if (name.value.trim() !== p.name) patch.name = name.value.trim();
      const satang = Math.round(parseFloat(price.value.replace(/,/g, "")) * 100);
      if (!Number.isFinite(satang) || satang < 0) throw new Error("ราคาไม่ถูกต้อง");
      if (satang !== p.price) patch.price = satang;
      if (onSale.checked !== p.on_sale) patch.on_sale = onSale.checked;
      if (!Object.keys(patch).length) { toast("ไม่มีอะไรเปลี่ยน"); return; }
      await api("POST", `/shops/${shopId}/changes`, { kind: "product", id: p.id, patch });
      toast("ส่งให้ร้านแล้ว");
      render();
    }) }, "บันทึก");
    pane.append(el("div", { class: p.active && p.on_sale ? "product" : "product off" },
      name, price, save,
      el("label", { class: "cat" }, onSale, " เปิดขาย", p.active ? "" : " · เก็บเข้าคลังแล้ว")));
  }
  if (!products.length) pane.append(el("p", { class: "muted" }, "ยังไม่มีเมนูจากร้าน เชื่อมเครื่องหลักในแท็บ \"ร้าน\" ก่อน"));
  return [pane];
}

async function shopView() {
  const shop = shops.find((s) => s.id === shopId);
  const [changes, backups] = await Promise.all([
    api("GET", `/shops/${shopId}/changes`),
    api("GET", `/shops/${shopId}/backups`),
  ]);
  const hub = el("section", { class: "pane" }, el("h2", {}, "เครื่องหลักในร้าน"));
  if (shop.hub) {
    hub.append(el("div", { class: "rows" },
      el("div", { class: "row" }, el("span", {}, shop.hub.name), el("span", { class: "badge good" }, "เชื่อมแล้ว")),
      el("div", { class: "row" }, el("span", { class: "muted" }, "ซิงก์ล่าสุด"), el("span", {}, when(shop.hub.last_seen_at)))));
  } else {
    hub.append(el("p", { class: "muted" }, "ยังไม่ได้เชื่อม"));
  }
  const codeBox = el("div");
  hub.append(el("p", { class: "muted" }, "ที่เครื่องหลัก เปิด ตั้งค่า → อุปกรณ์และคลาวด์ แล้วใส่ที่อยู่เว็บนี้และรหัสด้านล่าง (ใช้ได้ 10 นาที ครั้งเดียว) การเชื่อมเครื่องใหม่จะยกเลิกเครื่องเดิม"),
    el("p", { class: "muted" }, `ที่อยู่: ${location.origin}`),
    codeBox,
    el("button", { class: "primary", onclick: () => guard(async () => {
      const r = await api("POST", `/shops/${shopId}/link-code`);
      codeBox.replaceChildren(el("div", { class: "code" }, `${r.code.slice(0, 4)} ${r.code.slice(4)}`),
        el("p", { class: "muted" }, `หมดอายุ ${when(r.expires_at)}`));
    }) }, "สร้างรหัสเชื่อมต่อ"));
  const label = (c) => {
    if (!c.applied_at) return el("span", { class: "badge" }, "รอร้านรับ");
    return c.outcome === "applied" ? el("span", { class: "badge good" }, "ใช้แล้ว") : el("span", { class: "badge bad" }, "ร้านไม่รับ");
  };
  const history = el("section", { class: "pane" }, el("h2", {}, "การแก้ไขจากเว็บ"),
    el("div", { class: "rows" }, ...changes.map((c) => el("div", { class: "row" },
      el("span", {}, `${c.name || c.patch.name || "สินค้าใหม่"} · `, Object.entries(c.patch).map(([k, v]) => ({
        price: `ราคา ${money(v)}`, name: `ชื่อ ${v}`, on_sale: v ? "เปิดขาย" : "ปิดขาย", active: v ? "นำกลับมาขาย" : "เก็บเข้าคลัง",
        category: `หมวด ${v}`, sku: `รหัส ${v}`,
      }[k] || `${k}: ${v}`)).join(", ")),
      label(c),
      el("span", { class: "sub" }, `${when(c.created_at)}${c.outcome && c.outcome !== "applied" ? ` · ${c.outcome}` : ""}`)))),
    changes.length ? null : el("p", { class: "muted" }, "ยังไม่มี"));
  const size = (n) => n >= 1048576 ? `${(n / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`;
  const download = (b) => guard(async () => {
    const res = await fetch(`/api/v1/shops/${shopId}/backups/${b.id}`, { headers: { Authorization: `Bearer ${token}` } });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const url = URL.createObjectURL(await res.blob());
    const name = (res.headers.get("Content-Disposition") || "").match(/filename="([^"]+)"/);
    const a = el("a", { href: url, download: name ? name[1] : "pos-heart.phcloud" });
    document.body.append(a); a.click(); a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 10000);
  });
  const saved = el("section", { class: "pane" }, el("h2", {}, "ข้อมูลสำรองที่เข้ารหัส"),
    el("p", { class: "muted" }, "เครื่องหลักสำรองทั้งร้านวันละครั้ง เข้ารหัสด้วยรหัสผ่านข้อมูลสำรองของร้านก่อนส่ง ระบบนี้อ่านไม่ได้ ถ้าเครื่องในร้านเสีย ดาวน์โหลดไฟล์ล่าสุดแล้วกู้คืนที่เครื่องใหม่: ตั้งค่า → ข้อมูลและสำรอง"),
    el("div", { class: "rows" }, ...backups.map((b) => el("div", { class: "row" },
      el("span", {}, when(b.created_at)),
      el("button", { class: "secondary", onclick: () => download(b) }, "ดาวน์โหลด"),
      el("span", { class: "sub" }, size(b.size))))),
    backups.length ? null : el("p", { class: "muted" }, "ยังไม่มี ตั้งรหัสผ่านข้อมูลสำรองที่เครื่องหลัก: ตั้งค่า → อุปกรณ์และคลาวด์"));
  const another = el("section", { class: "pane" }, el("h2", {}, "ร้านอื่น"),
    el("button", { class: "secondary", onclick: () => { $("view").replaceChildren(newShopView()); } }, "เพิ่มร้าน"));
  return [hub, saved, history, another];
}

$("tabs").addEventListener("click", (ev) => {
  const b = ev.target.closest("button");
  if (b) { tab = b.dataset.tab; render(); }
});
$("shop").addEventListener("change", (ev) => { shopId = ev.target.value; render(); });
$("out").addEventListener("click", () => guard(async () => {
  await api("DELETE", "/sessions").catch(() => {});
  token = null; write(KEY, null); render();
}));
render();
