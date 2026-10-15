//! The iPad web app over loopback: setup page and profile on HTTP, the app
//! and its API on HTTPS trusted through the shop CA, pairing, a table bill
//! sent to the kitchen, change long-polling and revocation.
use pos_core::Pos;
use pos_domain::{DiningTable, ProductInput};
use pos_hub::{Executor, Hub, Job, tls::Identity, webtls::WebIdentity};
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned, crypto::ring};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{Arc, Mutex, mpsc},
    thread,
};
use uuid::Uuid;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

type Task = Box<dyn FnOnce(&mut Pos, &mut Executor) + Send>;
enum Owner {
    Job(Job),
    Do(Task),
}
fn owner(
    mut pos: Pos,
) -> (
    pos_hub::Submit,
    mpsc::Sender<Owner>,
    thread::JoinHandle<Pos>,
) {
    let (tx, rx) = mpsc::channel::<Owner>();
    let handle = thread::spawn(move || {
        let mut executor = Executor::default();
        let mut now = 1_760_000_000;
        while let Ok(item) = rx.recv() {
            now += 1;
            match item {
                Owner::Job(job) => {
                    executor.run(&mut pos, job, now);
                }
                Owner::Do(f) => f(&mut pos, &mut executor),
            }
        }
        pos
    });
    let jobs = Mutex::new(tx.clone());
    let submit: pos_hub::Submit =
        Arc::new(move |job| jobs.lock().is_ok_and(|t| t.send(Owner::Job(job)).is_ok()));
    (submit, tx, handle)
}

struct Web {
    https: SocketAddr,
    http: SocketAddr,
    tls: Arc<ClientConfig>,
}
fn split(raw: &[u8]) -> Result<(u16, Vec<u8>)> {
    let end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("no header end")?;
    let head = std::str::from_utf8(&raw[..end])?;
    let status = head.split(' ').nth(1).ok_or("no status")?.parse::<u16>()?;
    Ok((status, raw[end + 4..].to_vec()))
}
fn request(method: &str, path: &str, auth: Option<&str>, body: Option<&Value>) -> Vec<u8> {
    let body = body.map(|b| b.to_string()).unwrap_or_default();
    let mut out = format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    if let Some(a) = auth {
        out.push_str(&format!("Authorization: Bearer {a}\r\n"));
    }
    out.push_str("\r\n");
    out.push_str(&body);
    out.into_bytes()
}
impl Web {
    fn plain(&self, path: &str) -> Result<(u16, Vec<u8>)> {
        let mut tcp = TcpStream::connect(self.http)?;
        tcp.write_all(&request("GET", path, None, None))?;
        let mut raw = Vec::new();
        tcp.read_to_end(&mut raw)?;
        split(&raw)
    }
    fn send(
        &self,
        method: &str,
        path: &str,
        auth: Option<&str>,
        body: Option<&Value>,
    ) -> Result<(u16, Value)> {
        let (status, body) = self.raw(method, path, auth, body)?;
        Ok((status, serde_json::from_slice(&body)?))
    }
    fn raw(
        &self,
        method: &str,
        path: &str,
        auth: Option<&str>,
        body: Option<&Value>,
    ) -> Result<(u16, Vec<u8>)> {
        let conn = ClientConnection::new(self.tls.clone(), "localhost".try_into()?)?;
        let mut stream = StreamOwned::new(conn, TcpStream::connect(self.https)?);
        stream.write_all(&request(method, path, auth, body))?;
        let mut raw = Vec::new();
        match stream.read_to_end(&mut raw) {
            Ok(_) => {}
            // The server closed after the answer without anything missing.
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof && !raw.is_empty() => {}
            Err(e) => return Err(e.into()),
        }
        split(&raw)
    }
}

#[test]
fn ipad_pairs_and_sends_a_table_bill() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("shop.sqlite3"))?;
    let product = pos.save_product(ProductInput {
        sku: "K1".into(),
        name: "ข้าวผัด".into(),
        price: "60".parse()?,
        ..Default::default()
    })?;
    let table = pos.save_table(&DiningTable {
        id: Uuid::new_v4(),
        zone: "ในร้าน".into(),
        name: "A1".into(),
        seats: 4,
        active: true,
    })?;
    let (submit, control, owner) = owner(pos);
    let mut hub = Hub::start(
        TcpListener::bind("127.0.0.1:0")?,
        &Identity::generate()?,
        submit,
    )?;
    let identity = WebIdentity::generate()?;
    let https = TcpListener::bind("127.0.0.1:0")?;
    let http = TcpListener::bind("127.0.0.1:0")?;
    let web = {
        let mut roots = RootCertStore::empty();
        roots.add(identity.ca_der().to_vec().into())?;
        Web {
            https: https.local_addr()?,
            http: http.local_addr()?,
            tls: Arc::new(
                ClientConfig::builder_with_provider(Arc::new(ring::default_provider()))
                    .with_safe_default_protocol_versions()?
                    .with_root_certificates(roots)
                    .with_no_client_auth(),
            ),
        }
    };
    hub.start_web(https, http, &identity, &[], "ร้านทดสอบ")?;

    // HTTP: setup page and the CA profile, nothing else.
    let (status, page) = web.plain("/setup")?;
    assert_eq!(status, 200);
    let page = String::from_utf8(page)?;
    assert!(page.contains("/ca.mobileconfig") && page.contains("ร้านทดสอบ"));
    let (status, profile) = web.plain("/ca.mobileconfig")?;
    assert_eq!(status, 200);
    let profile = String::from_utf8(profile)?;
    assert!(profile.contains("com.apple.security.root"));
    assert!(profile.contains(&pos_hub::webtls::base64(identity.ca_der())));
    assert_eq!(web.plain("/api/state")?.0, 404);

    // HTTPS: the app, then pairing.
    let (status, index) = web.raw("GET", "/", None, None)?;
    assert_eq!(status, 200);
    assert!(String::from_utf8(index)?.contains("/app.js"));
    let (status, _) = web.send(
        "POST",
        "/api/pair",
        None,
        Some(&json!({"code": "000000", "name": "iPad"})),
    )?;
    assert_eq!(status, 403, "no pairing open");
    let code = hub.open_pairing();
    let (status, paired) = web.send(
        "POST",
        "/api/pair",
        None,
        Some(&json!({"code": code, "name": "iPad โซน A"})),
    )?;
    assert_eq!(status, 200);
    let auth = format!(
        "{}.{}",
        paired["device"].as_str().ok_or("device")?,
        paired["token"].as_str().ok_or("token")?
    );
    assert_eq!(web.send("GET", "/api/state", None, None)?.0, 401);
    assert_eq!(
        web.send("GET", "/api/state", Some("x.y"), None)?.0,
        401,
        "unknown device"
    );

    let (status, state) = web.send("GET", "/api/state", Some(&auth), None)?;
    assert_eq!(status, 200);
    assert_eq!(state["products"][0]["name"], "ข้าวผัด");
    assert_eq!(state["tables"][0]["name"], "A1");
    let before = state["total"].as_u64().ok_or("total")?;

    // Another request waiting for changes hears about the send.
    let poll = {
        let auth = auth.clone();
        let web = Web {
            https: web.https,
            http: web.http,
            tls: web.tls.clone(),
        };
        thread::spawn(move || {
            web.send(
                "GET",
                &format!("/api/changes?since={before}"),
                Some(&auth),
                None,
            )
            .map(|(_, v)| v["total"].as_u64())
            .ok()
            .flatten()
        })
    };
    thread::sleep(std::time::Duration::from_millis(300));
    let cart = json!({
        "id": Uuid::new_v4(), "label": "A1", "table": table.id, "guests": 2,
        "service_charge": true,
        "lines": [{"product_id": product.id, "quantity": 2, "note": "ไม่เผ็ด", "options": []}],
    });
    let (status, answer) = web.send(
        "POST",
        "/api/call",
        Some(&auth),
        Some(&json!({"id": Uuid::new_v4(), "request": {"SendKitchen": cart}})),
    )?;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["ok"]["KitchenSent"][1]["lines"][0]["quantity"], 2);
    let heard = poll.join().map_err(|_| "poll panicked")?;
    assert!(heard.is_some_and(|t| t > before), "long poll answered");

    // A business refusal is an error answer, not a failed request.
    let other = json!({
        "id": Uuid::new_v4(), "label": "A1", "table": table.id, "service_charge": true,
        "lines": [{"product_id": product.id, "quantity": 1, "note": "", "options": []}],
    });
    let (status, answer) = web.send(
        "POST",
        "/api/call",
        Some(&auth),
        Some(&json!({"id": Uuid::new_v4(), "request": {"Hold": other}})),
    )?;
    assert_eq!(status, 200);
    assert_eq!(answer["error"]["kind"], "TableBusy");

    let (_, state) = web.send("GET", "/api/state", Some(&auth), None)?;
    assert_eq!(state["held"][0]["table"], json!(table.id));
    assert_eq!(state["held"][0]["sent"][0]["quantity"], 2);

    // The counter changes the bill: the iPad's older copy is refused and
    // the reloaded one is accepted.
    let stale = state["held"][0].clone();
    let (done_tx, done_rx) = mpsc::channel();
    control.send(Owner::Do(Box::new(move |pos, _| {
        let result = (|| -> pos_core::Result<()> {
            let mut bill = pos.held()?.remove(0);
            bill.lines[0].quantity = 3;
            pos.hold(&bill, 1_900_000_000)
        })();
        let _ = done_tx.send(result);
    })))?;
    done_rx.recv()??;
    let (_, answer) = web.send(
        "POST",
        "/api/call",
        Some(&auth),
        Some(&json!({"id": Uuid::new_v4(), "request": {"Hold": stale}})),
    )?;
    assert_eq!(answer["error"]["kind"], "BillChanged");
    let (_, state) = web.send("GET", "/api/state", Some(&auth), None)?;
    let fresh = state["held"][0].clone();
    assert_eq!(fresh["lines"][0]["quantity"], 3);
    let (_, answer) = web.send(
        "POST",
        "/api/call",
        Some(&auth),
        Some(&json!({"id": Uuid::new_v4(), "request": {"Hold": fresh}})),
    )?;
    assert!(answer["ok"]["Held"].is_string(), "{answer}");

    // Revoked: the cached token stops working at once.
    let device = Uuid::parse_str(paired["device"].as_str().ok_or("device")?)?;
    let (done_tx, done_rx) = mpsc::channel();
    control.send(Owner::Do(Box::new(move |pos, executor| {
        let _ = done_tx.send(pos.revoke_device(device, 2_000_000_000));
        executor.forget(device);
    })))?;
    done_rx.recv()??;
    hub.disconnect(device);
    assert_eq!(web.send("GET", "/api/state", Some(&auth), None)?.0, 401);

    drop(hub);
    drop(control);
    let mut pos = owner.join().map_err(|_| "owner panicked")?;
    assert_eq!(pos.kitchen_tickets()?.len(), 1);
    Ok(())
}
