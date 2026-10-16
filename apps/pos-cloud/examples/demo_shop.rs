//! Fills a local cloud with a fictional shop for trying the dashboard:
//! creates a throwaway POS database with a week of sample sales, links it
//! with a dashboard code and uploads its outbox.
//!
//!   cargo run -p pos-cloud --example demo_shop -- http://127.0.0.1:8080 ABCD2345
use pos_domain::{Cart, CartLine, PaymentMethod, PaymentRequest, ProductInput};
use serde_json::{Value, json};
use uuid::Uuid;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn post(url: &str, auth: Option<&str>, body: Value) -> Result<Value> {
    let mut r = ureq::post(url);
    if let Some(a) = auth {
        r = r.header("Authorization", &format!("Bearer {a}"));
    }
    Ok(r.send_json(body)?.body_mut().read_json()?)
}

fn main() -> Result {
    let mut args = std::env::args().skip(1);
    let server = args.next().ok_or("usage: demo_shop SERVER CODE")?;
    let code = args.next().ok_or("usage: demo_shop SERVER CODE")?;
    let dir = tempfile::tempdir()?;
    let mut pos = pos_core::Pos::open(dir.path().join("demo.sqlite3"))?;
    let menu = [
        ("D1", "ชาไทยเย็น", "เครื่องดื่ม", "45"),
        ("D2", "กาแฟเย็น", "เครื่องดื่ม", "50"),
        ("D3", "น้ำเปล่า", "เครื่องดื่ม", "15"),
        ("F1", "ข้าวผัดกุ้ง", "อาหาร", "80"),
        ("F2", "กะเพราหมูสับไข่ดาว", "อาหาร", "70"),
        ("F3", "ผัดไทยกุ้งสด", "อาหาร", "90"),
        ("S1", "บัวลอยไข่หวาน", "ของหวาน", "40"),
    ];
    let mut products = Vec::new();
    for (sku, name, category, price) in menu {
        products.push(pos.save_product(ProductInput {
            sku: sku.into(),
            name: name.into(),
            category: category.into(),
            price: price.parse()?,
            ..Default::default()
        })?);
    }
    // A week of days, 12–30 bills each between 10:00 and 20:00 Bangkok.
    let today = time::OffsetDateTime::now_utc().unix_timestamp();
    let midnight = today - (today + 7 * 3600).rem_euclid(86_400);
    let mut seed = 7_u64;
    let mut next = |n: u64| {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        (seed >> 33) % n
    };
    for day in (0..7).rev() {
        let open = midnight - day * 86_400 + 10 * 3600;
        // One drawer shift per day, closed by the next day's opening.
        let shift = match pos.active_shift()? {
            Some(s) => s.id,
            None => {
                let id = Uuid::new_v4();
                pos.open_shift(id, "500".parse()?, open)?;
                id
            }
        };
        let bills = 12 + next(19);
        for b in 0..bills {
            let at = open + i64::try_from(b * 36_000 / bills + next(600))?;
            if at > today {
                break;
            }
            let mut cart = Cart::new();
            let mut total = 0;
            for _ in 0..1 + next(3) {
                let p = &products[usize::try_from(next(products.len() as u64))?];
                let quantity = 1 + u32::try_from(next(2))?;
                total += p.price.satang() * i64::from(quantity);
                cart.lines.push(CartLine {
                    product_id: p.id,
                    quantity,
                    ..Default::default()
                });
            }
            let method = match next(10) {
                0..=4 => PaymentMethod::Cash,
                5..=8 => PaymentMethod::Transfer,
                _ => PaymentMethod::Card,
            };
            pos.checkout_with_payment(
                &cart,
                // Transfers and cards are the exact total (no service/VAT here).
                PaymentRequest {
                    method,
                    amount: pos_domain::Money::from_satang(total)?,
                },
                Some(shift),
                at,
            )?;
        }
    }
    let linked = post(
        &format!("{server}/api/v1/hub/link"),
        None,
        json!({"code": code, "name": "เครื่องหลัก (ตัวอย่าง)"}),
    )?;
    let auth = format!(
        "{}.{}",
        linked["hub"].as_str().ok_or("link refused")?,
        linked["token"].as_str().ok_or("link refused")?
    );
    loop {
        let items = pos.outbox_batch(500)?;
        if items.is_empty() {
            break;
        }
        let acked: Vec<i64> = serde_json::from_value(
            post(
                &format!("{server}/api/v1/hub/upload"),
                Some(&auth),
                json!({ "items": items }),
            )?["acked"]
                .clone(),
        )?;
        pos.outbox_ack(&acked)?;
    }
    println!(
        "uploaded the demo shop to {}",
        linked["shop_name"].as_str().unwrap_or_default()
    );
    Ok(())
}
