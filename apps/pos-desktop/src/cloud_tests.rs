//! The main register's cloud sync end to end: the real worker and a real
//! `pos-cloud` server on PostgreSQL. A sale reaches the owner's report and
//! an owner's price edit reaches the shop. Needs
//! `POS_CLOUD_TEST_DATABASE_URL` (see apps/pos-cloud/tests/api.rs); skipped
//! without it.
use super::*;
use crate::worker::{Command, Event, Worker};
use serde_json::json;
use std::time::Instant;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn wait_for(worker: &Worker, mut want: impl FnMut(&Event) -> bool) -> Result {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Ok(e) = worker.rx.recv_timeout(Duration::from_millis(100))
            && want(&e.event)
        {
            return Ok(());
        }
    }
    Err("timed out waiting for the worker".into())
}
fn until(mut done: impl FnMut() -> bool) -> Result {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if done() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err("timed out".into())
}
fn post(url: &str, token: Option<&str>, body: Value) -> Result<Value> {
    let mut r = agent().post(url);
    if let Some(t) = token {
        r = r.header("Authorization", &format!("Bearer {t}"));
    }
    Ok(answer(r.send_json(body))?)
}
fn get(url: &str, token: &str) -> Result<Value> {
    Ok(answer(
        agent()
            .get(url)
            .header("Authorization", &format!("Bearer {token}"))
            .call(),
    )?)
}

#[test]
fn sales_go_up_and_owner_edits_come_down() -> Result {
    let Ok(admin) = std::env::var("POS_CLOUD_TEST_DATABASE_URL") else {
        eprintln!("skipped: set POS_CLOUD_TEST_DATABASE_URL to run the cloud sync test");
        return Ok(());
    };
    // A throwaway database and server.
    let database = format!("pos_cloud_sync_{}", Uuid::new_v4().simple());
    let (base_url, _) = admin.rsplit_once('/').ok_or("database URL")?;
    let url = format!("{base_url}/{database}");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let server = runtime
        .block_on(async {
            let admin_pool = pos_cloud::pool(&admin)?;
            admin_pool
                .get()
                .await?
                .batch_execute(&format!("CREATE DATABASE {database}"))
                .await?;
            let pool = pos_cloud::pool(&url)?;
            pos_cloud::migrate(&pool).await?;
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
            let server = format!("http://{}", listener.local_addr()?);
            tokio::spawn(pos_cloud::serve(listener, pos_cloud::AppState { pool }));
            Ok::<_, pos_cloud::BoxError>(server)
        })
        .map_err(|e| e.to_string())?;
    let api = |path: &str| format!("{server}/api/v1{path}");

    // The owner signs up, creates the shop and a link code.
    let owner = post(
        &api("/accounts"),
        None,
        json!({"email": "owner@example.com", "password": "a long enough password"}),
    )?["token"]
        .as_str()
        .ok_or("token")?
        .to_owned();
    let shop = post(&api("/shops"), Some(&owner), json!({"name": "ร้านทดสอบ"}))?["id"]
        .as_str()
        .ok_or("shop")?
        .to_owned();
    let code = post(
        &api(&format!("/shops/{shop}/link-code")),
        Some(&owner),
        json!(null),
    )?["code"]
        .as_str()
        .ok_or("code")?
        .to_owned();

    // The shop sells before it is linked: those records go up too.
    let dir = tempfile::tempdir()?;
    let worker = Worker::start(
        dir.path().join("pos-heart.sqlite3"),
        eframe::egui::Context::default(),
    )?;
    wait_for(&worker, |e| matches!(e, Event::Ready(_)))?;
    assert!(worker.send(Command::Save(pos_domain::ProductInput {
        sku: "T1".into(),
        name: "ชาไทย".into(),
        price: "45".parse()?,
        ..Default::default()
    })));
    let mut product = None;
    wait_for(&worker, |e| match e {
        Event::Saved(p) => {
            product = Some(p.id);
            true
        }
        _ => false,
    })?;
    let product = product.ok_or("product")?;
    let shift = Uuid::new_v4();
    assert!(worker.send(Command::OpenShift(shift, "0".parse()?)));
    wait_for(&worker, |e| matches!(e, Event::ShiftOpened))?;
    let mut cart = pos_domain::Cart::new();
    cart.lines.push(pos_domain::CartLine {
        product_id: product,
        quantity: 2,
        ..Default::default()
    });
    assert!(worker.send(Command::Pay(
        cart,
        pos_domain::PaymentRequest {
            method: pos_domain::PaymentMethod::Cash,
            amount: "100".parse()?,
        },
        shift
    )));
    wait_for(&worker, |e| matches!(e, Event::Paid(_)))?;

    // Link and sync: the outbox empties and the owner sees the sale.
    let file = link(&server, &code, "เครื่องหลัก")?;
    let file_copy = file.clone();
    assert_eq!(file.shop_name, "ร้านทดสอบ");
    let sync = Syncer::start(file, worker.cloud_submit(), dir.path().to_path_buf(), || {});
    until(|| {
        let s = sync.status();
        s.last_sync.is_some() && s.pending == Some(0)
    })?;
    let today = pos_service::bangkok_date(time::OffsetDateTime::now_utc().unix_timestamp());
    let report = get(
        &api(&format!("/shops/{shop}/summary?from={today}&to={today}")),
        &owner,
    )?;
    assert_eq!(report["sale_count"], 1);
    assert_eq!(report["sales"], 9000);

    // The owner changes the price; the next sync applies it here.
    post(
        &api(&format!("/shops/{shop}/changes")),
        Some(&owner),
        json!({"kind": "product", "id": product, "patch": {"price": 5000}}),
    )?;
    sync.wake();
    until(|| sync.status().applied == 1)?;
    let history = get(&api(&format!("/shops/{shop}/changes")), &owner)?;
    assert_eq!(history[0]["outcome"], "applied");
    // The counter hears about it and the cloud gets the new product state.
    wait_for(&worker, |e| matches!(e, Event::DevicesChanged))?;
    until(|| sync.status().pending == Some(0) && !sync.status().syncing)?;
    let catalog = get(&api(&format!("/shops/{shop}/products")), &owner)?;
    assert_eq!(catalog["products"][0]["price"], 5000);
    // Encrypted backup: set a password, back up now, download as the owner,
    // decrypt and the recovery bundle checks out.
    drop(sync);
    let mut file = CloudFile::load(dir.path()).unwrap_or(file_copy);
    let key = crate::cloud_backup::Key::new("shop backup password")?;
    let (salt, hex) = key.to_hex();
    file.backup_salt = Some(salt);
    file.backup_key = Some(hex);
    let sync = Syncer::start(file, worker.cloud_submit(), dir.path().to_path_buf(), || {});
    sync.backup_now();
    until(|| sync.status().last_backup.is_some())?;
    assert_eq!(sync.status().backup_error, None);
    let backups = get(&api(&format!("/shops/{shop}/backups")), &owner)?;
    let id = backups[0]["id"].as_str().ok_or("backup")?.to_owned();
    let mut response = agent()
        .get(&api(&format!("/shops/{shop}/backups/{id}")))
        .header("Authorization", &format!("Bearer {owner}"))
        .call()?;
    let sealed = response
        .body_mut()
        .with_config()
        .limit(1 << 30)
        .read_to_vec()?;
    let restored = dir.path().join("restored.posbackup");
    assert!(crate::cloud_backup::open(&sealed, "not the password", &restored).is_err());
    crate::cloud_backup::open(&sealed, "shop backup password", &restored)?;
    let review = crate::backup::inspect(&restored)?;
    assert_eq!(review.manifest.info.sales, 1);
    drop(sync);
    drop(worker);
    runtime.block_on(async {
        if let Ok(pool) = pos_cloud::pool(&admin)
            && let Ok(db) = pool.get().await
        {
            let _ = db
                .batch_execute(&format!("DROP DATABASE IF EXISTS {database} WITH (FORCE)"))
                .await;
        }
    });
    Ok(())
}
