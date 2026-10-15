//! The iPad web app's server: a small HTTP/1.1 server on plain threads.
//! HTTPS serves the app and its JSON API; plain HTTP only serves the setup
//! page and the certificate profile (needed before the iPad trusts HTTPS).
//! One request per connection; bodies at most 1 MiB.
use crate::{Done, Shared, Submit, Work, ask, token_hash, wire::WireError};
use pos_core::DeviceKind;
use pos_service::{Request, Versions};
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, atomic::Ordering},
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

const MAX_BODY: usize = 1024 * 1024;
const MAX_HEAD: usize = 16 * 1024;
/// How long `/api/changes` waits for a change before answering anyway.
const LONG_POLL: Duration = Duration::from_secs(25);

const INDEX: &str = include_str!("../web/index.html");
const APP_JS: &str = include_str!("../web/app.js");
const APP_CSS: &str = include_str!("../web/app.css");

pub(crate) struct Site {
    pub shared: Arc<Shared>,
    pub submit: Submit,
    pub profile: String,
    pub ca: Vec<u8>,
    pub shop: String,
    pub https_port: u16,
}

pub(crate) fn listen(
    listener: TcpListener,
    tls: Option<Arc<ServerConfig>>,
    site: Arc<Site>,
) -> std::io::Result<thread::JoinHandle<()>> {
    listener.set_nonblocking(true)?;
    thread::Builder::new()
        .name("pos-hub-web".into())
        .spawn(move || {
            while !site.shared.stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let site = site.clone();
                        let tls = tls.clone();
                        let _ = thread::Builder::new()
                            .name("pos-hub-web-conn".into())
                            .spawn(move || connection(stream, tls, &site));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(50));
                    }
                    Err(_) => thread::sleep(Duration::from_millis(200)),
                }
            }
        })
}

fn connection(mut tcp: TcpStream, tls: Option<Arc<ServerConfig>>, site: &Site) {
    if tcp.set_nonblocking(false).is_err()
        || tcp.set_read_timeout(Some(Duration::from_secs(15))).is_err()
        || tcp.set_nodelay(true).is_err()
    {
        return;
    }
    match tls {
        Some(config) => {
            let Ok(mut conn) = ServerConnection::new(config) else {
                return;
            };
            if crate::handshake(&mut conn, &mut tcp).is_err() {
                return;
            }
            let _ = tcp.set_read_timeout(Some(Duration::from_secs(15)));
            let mut stream = StreamOwned::new(conn, tcp);
            if let Some(request) = read_request(&mut stream) {
                let response = route_https(site, &request);
                let _ = stream.write_all(&response);
                let _ = stream.flush();
                stream.conn.send_close_notify();
                let _ = stream.flush();
            }
        }
        None => {
            if let Some(request) = read_request(&mut tcp) {
                let response = route_http(site, &request);
                let _ = tcp.write_all(&response);
                let _ = tcp.flush();
            }
        }
    }
}

struct HttpRequest {
    method: String,
    path: String,
    query: HashMap<String, String>,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

fn read_request(stream: &mut impl Read) -> Option<HttpRequest> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        if buf.len() > MAX_HEAD {
            return None;
        }
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = std::str::from_utf8(&buf[..head_end]).ok()?;
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let method = first.next()?.to_owned();
    let target = first.next()?.to_owned();
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_owned());
        }
    }
    let length: usize = headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if length > MAX_BODY {
        return None;
    }
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < length {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(length);
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_owned(), q),
        None => (target.clone(), ""),
    };
    let query = query
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
    Some(HttpRequest {
        method,
        path,
        query,
        headers,
        body,
    })
}

fn response(status: u16, kind: &str, body: &[u8], extra: &[(&str, String)]) -> Vec<u8> {
    let reason = match status {
        200 => "OK",
        302 => "Found",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Error",
    };
    let mut head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\n",
        body.len()
    );
    for (k, v) in extra {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    let mut out = head.into_bytes();
    out.extend_from_slice(body);
    out
}
fn json<T: Serialize>(status: u16, value: &T) -> Vec<u8> {
    match serde_json::to_vec(value) {
        Ok(body) => response(status, "application/json; charset=utf-8", &body, &[]),
        Err(_) => response(500, "text/plain", b"error", &[]),
    }
}
fn error(status: u16, kind: &str, message: &str) -> Vec<u8> {
    json(
        status,
        &ApiAnswer::Error {
            error: WireError::new(kind, message),
        },
    )
}

#[derive(Serialize)]
#[serde(untagged)]
enum ApiAnswer {
    Ok { ok: Box<pos_service::Reply> },
    Error { error: WireError },
}
#[derive(Deserialize)]
struct PairBody {
    code: String,
    name: String,
}
#[derive(Serialize)]
struct Paired {
    device: Uuid,
    token: String,
    register: Uuid,
}
#[derive(Deserialize)]
struct CallBody {
    id: Uuid,
    request: Box<Request>,
}
#[derive(Serialize)]
struct Changes {
    versions: Versions,
    total: u64,
}
fn total(v: &Versions) -> u64 {
    v.0.iter().fold(0u64, |a, b| a.saturating_add(*b))
}

/// Plain HTTP: setup page and certificate only; everything else moves to HTTPS.
fn route_http(site: &Site, req: &HttpRequest) -> Vec<u8> {
    match req.path.as_str() {
        "/ca.mobileconfig" => response(
            200,
            "application/x-apple-aspen-config",
            site.profile.as_bytes(),
            &[(
                "Content-Disposition",
                "attachment; filename=\"pos-heart.mobileconfig\"".into(),
            )],
        ),
        "/ca.der" => response(200, "application/x-x509-ca-cert", &site.ca, &[]),
        "/setup" | "/" => response(
            200,
            "text/html; charset=utf-8",
            setup_page(site, req).as_bytes(),
            &[],
        ),
        _ => response(404, "text/plain", b"not found", &[]),
    }
}
fn host(req: &HttpRequest) -> String {
    req.headers
        .get("host")
        .map(|h| h.split(':').next().unwrap_or_default().to_owned())
        .unwrap_or_else(|| "posheart.local".into())
}
fn setup_page(site: &Site, req: &HttpRequest) -> String {
    let app = format!("https://{}:{}/", host(req), site.https_port);
    let shop = html_escape(&site.shop);
    format!(
        r#"<!doctype html><html lang="th"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>POS Heart · ตั้งค่า iPad</title>
<style>body{{font-family:-apple-system,"Sukhumvit Set",sans-serif;background:#2a2b2f;color:#ecebe7;margin:0;padding:24px;line-height:1.6}}main{{max-width:560px;margin:auto;background:#f2f2ef;color:#1c1d1f;border-radius:12px;padding:24px}}a.b{{display:block;text-align:center;background:#f0b232;color:#141310;text-decoration:none;font-weight:600;padding:14px;border-radius:10px;margin:12px 0}}ol{{padding-left:20px}}small{{color:#5c5e5a}}</style></head>
<body><main><h2>ตั้งค่า iPad สำหรับ {shop}</h2>
<p>ทำครั้งเดียวต่อเครื่อง เพื่อให้ iPad เชื่อมต่อเครื่องหลักแบบเข้ารหัส</p>
<ol><li>กดปุ่มด้านล่างเพื่อดาวน์โหลดโปรไฟล์ แล้วกด <b>อนุญาต</b></li>
<li>เปิด <b>การตั้งค่า → ดาวน์โหลดโปรไฟล์แล้ว</b> แล้วกด <b>ติดตั้ง</b></li>
<li>เปิด <b>การตั้งค่า → ทั่วไป → เกี่ยวกับ → การตั้งค่าความเชื่อถือใบรับรอง</b> แล้วเปิดสวิตช์ <b>POS Heart</b></li>
<li>เปิดแอปรับออเดอร์ แล้วกด <b>แชร์ → เพิ่มไปยังหน้าจอโฮม</b></li></ol>
<a class="b" href="/ca.mobileconfig">1. ดาวน์โหลดโปรไฟล์ใบรับรอง</a>
<a class="b" href="{app}">2. เปิดแอปรับออเดอร์</a>
<small>Set up this iPad once: download and install the profile, enable full trust for POS Heart in Settings → General → About → Certificate Trust Settings, then open the ordering app and add it to the Home Screen.</small>
</main></body></html>"#
    )
}
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn route_https(site: &Site, req: &HttpRequest) -> Vec<u8> {
    let csp = (
        "Content-Security-Policy",
        "default-src 'self'; img-src 'self' data: blob:; style-src 'self'; script-src 'self'; connect-src 'self'".to_owned(),
    );
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/") | ("GET", "/index.html") => {
            response(200, "text/html; charset=utf-8", INDEX.as_bytes(), &[csp])
        }
        ("GET", "/app.js") => response(
            200,
            "text/javascript; charset=utf-8",
            APP_JS.as_bytes(),
            &[],
        ),
        ("GET", "/app.css") => response(200, "text/css; charset=utf-8", APP_CSS.as_bytes(), &[]),
        ("GET", "/ca.mobileconfig") | ("GET", "/ca.der") | ("GET", "/setup") => {
            route_http(site, req)
        }
        ("POST", "/api/pair") => pair(site, req),
        ("POST", "/api/call") => call(site, req),
        ("GET", "/api/changes") => changes(site, req),
        ("GET", "/api/state") => state(site, req),
        ("GET", "/api/shop") => json(200, &serde_json::json!({ "name": site.shop })),
        (_, p) if p.starts_with("/api/") => error(405, "BadMessage", "unsupported request"),
        _ => response(404, "text/plain", b"not found", &[]),
    }
}

fn pair(site: &Site, req: &HttpRequest) -> Vec<u8> {
    let Ok(body) = serde_json::from_slice::<PairBody>(&req.body) else {
        return error(400, "BadMessage", "expected {code, name}");
    };
    let code: String = body.code.chars().filter(char::is_ascii_digit).collect();
    if !site.shared.take_code(&code) {
        return json(
            403,
            &serde_json::json!({ "error": { "kind": "WrongCode", "message": "wrong or expired code" } }),
        );
    }
    let device = Uuid::new_v4();
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let hash = token_hash(&token);
    match ask(
        &site.submit,
        device,
        Work::Pair {
            name: body.name,
            kind: DeviceKind::Ordering,
            token_hash: hash.clone(),
        },
    ) {
        Ok(Done::Paired { register }) => {
            if let Ok(mut web) = site.shared.web.lock() {
                web.insert(device, (hash, register));
            }
            json(
                200,
                &Paired {
                    device,
                    token,
                    register,
                },
            )
        }
        Ok(_) => error(500, "Protocol", "unexpected answer"),
        Err(e) => json(400, &ApiAnswer::Error { error: e }),
    }
}

/// `Authorization: Bearer <device>.<token>` → (device, register).
fn authorize(site: &Site, req: &HttpRequest) -> Option<(Uuid, Uuid)> {
    let value = req.headers.get("authorization")?;
    let (device, token) = value.strip_prefix("Bearer ")?.split_once('.')?;
    let device = Uuid::parse_str(device).ok()?;
    let hash = token_hash(token);
    if let Ok(web) = site.shared.web.lock()
        && let Some((known, register)) = web.get(&device)
        && *known == hash
    {
        return Some((device, *register));
    }
    match ask(
        &site.submit,
        device,
        Work::Login {
            token_hash: hash.clone(),
        },
    ) {
        Ok(Done::Login(Some(register))) => {
            if let Ok(mut web) = site.shared.web.lock() {
                web.insert(device, (hash, register));
            }
            Some((device, register))
        }
        _ => None,
    }
}

fn call(site: &Site, req: &HttpRequest) -> Vec<u8> {
    let Some((device, register)) = authorize(site, req) else {
        return error(401, "NotPaired", "pair this iPad again");
    };
    let Ok(body) = serde_json::from_slice::<CallBody>(&req.body) else {
        return error(400, "BadMessage", "expected {id, request}");
    };
    let _ = body.id;
    let request = *body.request;
    match ask(
        &site.submit,
        device,
        Work::Call {
            register,
            request: Box::new(request.clone()),
        },
    ) {
        Ok(Done::Reply(reply)) => {
            site.shared.notify(&request);
            json(200, &ApiAnswer::Ok { ok: reply })
        }
        Ok(_) => error(500, "Protocol", "unexpected answer"),
        Err(e) => json(200, &ApiAnswer::Error { error: e }),
    }
}

fn changes(site: &Site, req: &HttpRequest) -> Vec<u8> {
    if authorize(site, req).is_none() {
        return error(401, "NotPaired", "pair this iPad again");
    }
    let since: u64 = req
        .query
        .get("since")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let started = Instant::now();
    loop {
        let versions = site
            .shared
            .versions
            .lock()
            .map(|v| v.clone())
            .unwrap_or_default();
        let now = total(&versions);
        if now != since || started.elapsed() > LONG_POLL || site.shared.stop.load(Ordering::SeqCst)
        {
            return json(
                200,
                &Changes {
                    versions,
                    total: now,
                },
            );
        }
        thread::sleep(Duration::from_millis(250));
    }
}

/// What the ordering app shows: the menu, tables, their open bills and
/// sign-in state, from one ordinary `Refresh` (lean JSON, no reports).
fn state(site: &Site, req: &HttpRequest) -> Vec<u8> {
    let Some((device, register)) = authorize(site, req) else {
        return error(401, "NotPaired", "pair this iPad again");
    };
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let query = pos_service::Query {
        products: pos_core::ProductQuery {
            limit: 500,
            ..Default::default()
        },
        sale_offset: 0,
        report: pos_service::Period::day(pos_service::bangkok_date(now)),
        shift_offset: 0,
        movement_shift: None,
        movement_offset: 0,
        stock_product: None,
    };
    let request = Request::Refresh(Box::new(query));
    match ask(
        &site.submit,
        device,
        Work::Call {
            register,
            request: Box::new(request),
        },
    ) {
        Ok(Done::Reply(reply)) => {
            let pos_service::Reply::Data(data) = *reply else {
                return error(500, "Protocol", "unexpected answer");
            };
            let products: Vec<_> = data
                .products
                .iter()
                .filter(|p| p.active && p.on_sale)
                .map(|p| {
                    serde_json::json!({
                        "id": p.id, "name": p.name, "category": p.category,
                        "price": p.price, "stock": p.stock, "options": p.options,
                        "kitchen": p.kitchen,
                    })
                })
                .collect();
            let staff: Vec<_> = data
                .staff
                .iter()
                .filter(|s| s.active)
                .map(|s| serde_json::json!({ "id": s.id, "name": s.name }))
                .collect();
            let versions = site
                .shared
                .versions
                .lock()
                .map(|v| v.clone())
                .unwrap_or_default();
            json(
                200,
                &serde_json::json!({
                    "shop": site.shop,
                    "products": products,
                    "categories": data.counts.categories.iter().map(|(n, _)| n).collect::<Vec<_>>(),
                    "tables": data.tables.iter().filter(|t| t.active).collect::<Vec<_>>(),
                    "held": data.held,
                    "staff_enabled": data.staff_enabled,
                    "staff": staff,
                    "signed_in": data.signed_in,
                    "total": total(&versions),
                }),
            )
        }
        Ok(_) => error(500, "Protocol", "unexpected answer"),
        Err(e) => json(200, &ApiAnswer::Error { error: e }),
    }
}
