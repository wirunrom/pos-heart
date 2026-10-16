//! The cloud API end to end against a real PostgreSQL: owner accounts,
//! linking a hub, uploading a real shop's outbox, the report, menu edits
//! round trip, isolation between owners and relinking.
//!
//! Needs `POS_CLOUD_TEST_DATABASE_URL` (an admin URL such as
//! `postgres://postgres:postgres@localhost:5432/postgres`); each run creates
//! and drops its own database. Without it the test only says it was skipped.
use pos_domain::{Cart, CartLine, PaymentMethod, PaymentRequest, ProductInput};
use serde_json::{Value, json};
use uuid::Uuid;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

/// A sale paid at 2025-10-09 15:53 Bangkok.
const PAID_AT: i64 = 1_760_000_000;

struct Server {
    base: String,
    admin: String,
    database: String,
    runtime: Option<tokio::runtime::Runtime>,
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(rt) = self.runtime.take() {
            rt.shutdown_background();
        }
        let (admin, database) = (self.admin.clone(), self.database.clone());
        // Best effort: a leftover test database is harmless.
        let _ = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            rt.block_on(async {
                let (client, conn) = tokio_postgres::connect(&admin, tokio_postgres::NoTls).await?;
                tokio::spawn(conn);
                client
                    .batch_execute(&format!("DROP DATABASE IF EXISTS {database} WITH (FORCE)"))
                    .await
            })?;
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
        })
        .join();
    }
}
fn start() -> Result<Option<Server>> {
    let Ok(admin) = std::env::var("POS_CLOUD_TEST_DATABASE_URL") else {
        eprintln!("skipped: set POS_CLOUD_TEST_DATABASE_URL to run the cloud API test");
        return Ok(None);
    };
    let database = format!("pos_cloud_test_{}", Uuid::new_v4().simple());
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let base = runtime
        .block_on(async {
            let (client, conn) = tokio_postgres::connect(&admin, tokio_postgres::NoTls).await?;
            tokio::spawn(conn);
            client
                .batch_execute(&format!("CREATE DATABASE {database}"))
                .await?;
            let mut config: tokio_postgres::Config = admin.parse()?;
            config.dbname(&database);
            let manager = deadpool_postgres::Manager::new(config, tokio_postgres::NoTls);
            let pool = deadpool_postgres::Pool::builder(manager)
                .max_size(8)
                .build()?;
            pos_cloud::migrate(&pool).await?;
            // Migrating again is a no-op.
            pos_cloud::migrate(&pool).await?;
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
            let base = format!("http://{}", listener.local_addr()?);
            tokio::spawn(async move {
                let _ =
                    axum::serve(listener, pos_cloud::router(pos_cloud::AppState { pool })).await;
            });
            Ok::<_, pos_cloud::BoxError>(base)
        })
        .map_err(|e| e.to_string())?;
    Ok(Some(Server {
        base,
        admin,
        database,
        runtime: Some(runtime),
    }))
}

/// (status, JSON body or Null).
fn call(method: &str, url: &str, token: Option<&str>, body: Option<Value>) -> Result<(u16, Value)> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .into();
    let auth = token.map(|t| format!("Bearer {t}"));
    let mut response = match (method, body) {
        ("GET", _) => {
            let mut r = agent.get(url);
            if let Some(a) = &auth {
                r = r.header("Authorization", a);
            }
            r.call()?
        }
        ("DELETE", _) => {
            let mut r = agent.delete(url);
            if let Some(a) = &auth {
                r = r.header("Authorization", a);
            }
            r.call()?
        }
        (_, body) => {
            let mut r = agent.post(url);
            if let Some(a) = &auth {
                r = r.header("Authorization", a);
            }
            r.send_json(body.unwrap_or(Value::Null))?
        }
    };
    let status = response.status().as_u16();
    let text = response.body_mut().read_to_string()?;
    Ok((status, serde_json::from_str(&text).unwrap_or(Value::Null)))
}

/// Posts raw bytes; (status, body bytes).
fn raw(method: &str, url: &str, token: &str, body: Option<&[u8]>) -> Result<(u16, Vec<u8>)> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .into();
    let auth = format!("Bearer {token}");
    let mut response = match (method, body) {
        ("POST", Some(b)) => agent
            .post(url)
            .header("Authorization", &auth)
            .header("Content-Type", "application/octet-stream")
            .send(b)?,
        _ => agent.get(url).header("Authorization", &auth).call()?,
    };
    let status = response.status().as_u16();
    Ok((status, response.body_mut().read_to_vec()?))
}

/// A real shop: one product, one sale, everything in the outbox.
fn shop_outbox() -> Result<(tempfile::TempDir, pos_core::Pos, Uuid)> {
    let dir = tempfile::tempdir()?;
    let mut pos = pos_core::Pos::open(dir.path().join("shop.sqlite3"))?;
    let product = pos.save_product(ProductInput {
        sku: "T1".into(),
        name: "ชาไทย".into(),
        category: "เครื่องดื่ม".into(),
        price: "45".parse()?,
        ..Default::default()
    })?;
    let shift = Uuid::new_v4();
    pos.open_shift(shift, "0".parse()?, PAID_AT - 60)?;
    let mut cart = Cart::new();
    cart.lines.push(CartLine {
        product_id: product.id,
        quantity: 2,
        ..Default::default()
    });
    pos.checkout_with_payment(
        &cart,
        PaymentRequest {
            method: PaymentMethod::Transfer,
            amount: "90".parse()?,
        },
        Some(shift),
        PAID_AT,
    )?;
    Ok((dir, pos, product.id))
}

#[test]
fn owners_hubs_uploads_reports_and_menu_edits() -> Result {
    let Some(server) = start()? else {
        return Ok(());
    };
    let api = |path: &str| format!("{}/api/v1{path}", server.base);

    // Accounts.
    let creds = json!({"email": "Owner@Example.com ", "password": "correct horse battery"});
    let (status, body) = call("POST", &api("/accounts"), None, Some(creds.clone()))?;
    assert_eq!(status, 200, "{body}");
    let owner = body["token"].as_str().ok_or("token")?.to_owned();
    assert_eq!(
        call("POST", &api("/accounts"), None, Some(creds.clone()))?.0,
        409
    );
    assert_eq!(
        call(
            "POST",
            &api("/accounts"),
            None,
            Some(json!({"email": "x@y.co", "password": "short"}))
        )?
        .0,
        400
    );
    assert_eq!(
        call(
            "POST",
            &api("/sessions"),
            None,
            Some(json!({"email": "owner@example.com", "password": "wrong password!"}))
        )?
        .0,
        401
    );
    let (status, _) = call("POST", &api("/sessions"), None, Some(creds))?;
    assert_eq!(status, 200);
    assert_eq!(call("GET", &api("/shops"), None, None)?.0, 401);

    // A shop and its link code.
    let (_, shop) = call(
        "POST",
        &api("/shops"),
        Some(&owner),
        Some(json!({"name": "ร้านทดสอบ"})),
    )?;
    let shop_id = shop["id"].as_str().ok_or("shop id")?.to_owned();
    let (_, code) = call(
        "POST",
        &api(&format!("/shops/{shop_id}/link-code")),
        Some(&owner),
        None,
    )?;
    let code = code["code"].as_str().ok_or("code")?.to_owned();
    assert_eq!(code.len(), 8);
    assert_eq!(
        call(
            "POST",
            &api("/hub/link"),
            None,
            Some(json!({"code": "WRONGONE", "name": "เครื่องหลัก"}))
        )?
        .0,
        403
    );
    let (status, linked) = call(
        "POST",
        &api("/hub/link"),
        None,
        Some(json!({"code": code.to_lowercase(), "name": "เครื่องหลัก"})),
    )?;
    assert_eq!(status, 200, "{linked}");
    assert_eq!(linked["shop_name"], "ร้านทดสอบ");
    let hub = format!(
        "{}.{}",
        linked["hub"].as_str().ok_or("hub")?,
        linked["token"].as_str().ok_or("hub token")?
    );
    assert_eq!(
        call(
            "POST",
            &api("/hub/link"),
            None,
            Some(json!({"code": code, "name": "again"}))
        )?
        .0,
        403,
        "a code works once"
    );

    // The real outbox goes up; every row is acknowledged; resending is harmless.
    let (_dir, mut pos, product) = shop_outbox()?;
    let items = pos.outbox_batch(1000)?;
    assert_eq!(
        call(
            "POST",
            &api("/hub/upload"),
            Some("x.y"),
            Some(json!({"items": []}))
        )?
        .0,
        401
    );
    let (status, acked) = call(
        "POST",
        &api("/hub/upload"),
        Some(&hub),
        Some(json!({ "items": items })),
    )?;
    assert_eq!(status, 200, "{acked}");
    let acked: Vec<i64> = serde_json::from_value(acked["acked"].clone())?;
    assert_eq!(acked.len(), items.len());
    assert_eq!(
        call(
            "POST",
            &api("/hub/upload"),
            Some(&hub),
            Some(json!({ "items": items }))
        )?
        .0,
        200
    );
    pos.outbox_ack(&acked)?;
    assert_eq!(pos.outbox_len()?, 0);

    // The owner's report and catalog.
    let (status, report) = call(
        "GET",
        &api(&format!(
            "/shops/{shop_id}/summary?from=2025-10-09&to=2025-10-09"
        )),
        Some(&owner),
        None,
    )?;
    assert_eq!(status, 200, "{report}");
    assert_eq!(report["sale_count"], 1);
    assert_eq!(report["sales"], 9000);
    assert_eq!(report["methods"]["transfer"]["sales"], 9000);
    assert_eq!(report["hourly"][15], 9000);
    assert_eq!(report["top_products"][0]["name"], "ชาไทย");
    let (_, empty) = call(
        "GET",
        &api(&format!(
            "/shops/{shop_id}/summary?from=2025-10-10&to=2025-10-10"
        )),
        Some(&owner),
        None,
    )?;
    assert_eq!(empty["sale_count"], 0);
    let (_, catalog) = call(
        "GET",
        &api(&format!("/shops/{shop_id}/products")),
        Some(&owner),
        None,
    )?;
    assert_eq!(catalog["products"][0]["name"], "ชาไทย");
    assert_eq!(catalog["categories"][0], "เครื่องดื่ม");

    // A menu edit travels to the hub and its outcome back to the owner.
    assert_eq!(
        call(
            "POST",
            &api(&format!("/shops/{shop_id}/changes")),
            Some(&owner),
            Some(json!({"kind": "product", "id": product, "patch": {"password": "x"}})),
        )?
        .0,
        400
    );
    let (status, change) = call(
        "POST",
        &api(&format!("/shops/{shop_id}/changes")),
        Some(&owner),
        Some(json!({"kind": "product", "id": product, "patch": {"price": 5000}})),
    )?;
    assert_eq!(status, 200, "{change}");
    let (_, pending) = call("GET", &api("/hub/changes"), Some(&hub), None)?;
    assert_eq!(pending[0]["patch"]["price"], 5000);
    let seq = pending[0]["seq"].as_i64().ok_or("seq")?;
    call(
        "POST",
        &api("/hub/changes/ack"),
        Some(&hub),
        Some(json!({"results": [{"seq": seq, "outcome": "applied"}]})),
    )?;
    let (_, pending) = call("GET", &api("/hub/changes"), Some(&hub), None)?;
    assert_eq!(pending, json!([]));
    let (_, history) = call(
        "GET",
        &api(&format!("/shops/{shop_id}/changes")),
        Some(&owner),
        None,
    )?;
    assert_eq!(history[0]["outcome"], "applied");
    let (_, shops) = call("GET", &api("/shops"), Some(&owner), None)?;
    assert_eq!(shops[0]["hub"]["name"], "เครื่องหลัก");
    assert_eq!(shops[0]["pending_changes"], 0);

    // Encrypted backups: only the container is accepted; the owner downloads it back.
    assert_eq!(
        raw("POST", &api("/hub/backups"), &hub, Some(b"plain database"))?.0,
        400
    );
    let sealed = [b"PHCLOUD1".as_slice(), &[7u8; 64]].concat();
    let (status, _) = raw("POST", &api("/hub/backups"), &hub, Some(&sealed))?;
    assert_eq!(status, 200);
    let (_, backups) = call(
        "GET",
        &api(&format!("/shops/{shop_id}/backups")),
        Some(&owner),
        None,
    )?;
    assert_eq!(backups[0]["size"], sealed.len());
    let backup = backups[0]["id"].as_str().ok_or("backup id")?.to_owned();
    let (status, bytes) = raw(
        "GET",
        &api(&format!("/shops/{shop_id}/backups/{backup}")),
        &owner,
        None,
    )?;
    assert_eq!((status, bytes), (200, sealed));

    // Another owner sees nothing of this shop.
    let (_, other) = call(
        "POST",
        &api("/accounts"),
        None,
        Some(json!({"email": "other@example.com", "password": "another long password"})),
    )?;
    let other = other["token"].as_str().ok_or("other token")?.to_owned();
    for path in [
        "summary?from=2025-10-09&to=2025-10-09",
        "products",
        "changes",
        "backups",
    ] {
        assert_eq!(
            call(
                "GET",
                &api(&format!("/shops/{shop_id}/{path}")),
                Some(&other),
                None
            )?
            .0,
            404
        );
    }

    // Relinking revokes the old hub; signing out ends the session.
    let (_, code) = call(
        "POST",
        &api(&format!("/shops/{shop_id}/link-code")),
        Some(&owner),
        None,
    )?;
    call(
        "POST",
        &api("/hub/link"),
        None,
        Some(json!({"code": code["code"], "name": "เครื่องใหม่"})),
    )?;
    assert_eq!(call("GET", &api("/hub/changes"), Some(&hub), None)?.0, 401);
    assert_eq!(
        call("DELETE", &api("/sessions"), Some(&owner), None)?.0,
        204
    );
    assert_eq!(call("GET", &api("/shops"), Some(&owner), None)?.0, 401);
    Ok(())
}
