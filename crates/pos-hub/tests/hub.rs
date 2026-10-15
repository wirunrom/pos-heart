//! A hub and devices over loopback: two registers selling the shared stock,
//! per-device sign-in, retries after a dropped connection and refusals.
use pos_core::{MAIN_REGISTER, Pos};
use pos_domain::{Cart, CartLine, PaymentMethod, PaymentRequest, ProductInput};
use pos_hub::{Client, ClientError, Executor, Hub, Job, wire::Refusal};
use pos_service::{Reply, Request, Topic};
use std::{
    net::TcpListener,
    sync::{Arc, Mutex, mpsc},
    thread,
};
use uuid::Uuid;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

/// The database owner: one thread running jobs in order, like the worker.
fn owner(mut pos: Pos) -> (pos_hub::Submit, thread::JoinHandle<Pos>) {
    let (tx, rx) = mpsc::channel::<Job>();
    let handle = thread::spawn(move || {
        let mut executor = Executor::default();
        let mut now = 1_760_000_000;
        while let Ok(job) = rx.recv() {
            now += 1;
            executor.run(&mut pos, job, now);
        }
        pos
    });
    let tx = Mutex::new(tx);
    let submit: pos_hub::Submit = Arc::new(move |job| tx.lock().is_ok_and(|t| t.send(job).is_ok()));
    (submit, handle)
}

#[test]
fn two_registers_sell_shared_stock_through_the_hub() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("shop.sqlite3"))?;
    let product = pos.save_product(ProductInput {
        sku: "A".into(),
        name: "น้ำดื่ม".into(),
        price: "10".parse()?,
        stock: Some(3),
        ..Default::default()
    })?;
    let second = Uuid::new_v4();
    pos.add_register(second, "เคาน์เตอร์ 2", 1)?;
    let (submit, owner) = owner(pos);
    let tablet = Uuid::new_v4();
    let register2 = Uuid::new_v4();
    let authorize: pos_hub::Authorize = Arc::new(move |device, token| {
        if device == tablet && token == "t-token" {
            Some(MAIN_REGISTER)
        } else if device == register2 && token == "r-token" {
            Some(second)
        } else {
            None
        }
    });
    let hub = Hub::start(TcpListener::bind("127.0.0.1:0")?, authorize, submit)?;
    let addr = hub.address();

    // Unknown devices and wrong tokens are refused.
    assert!(matches!(
        Client::connect(addr, Uuid::new_v4(), "x"),
        Err(ClientError::Refused(Refusal::NotPaired))
    ));
    assert!(matches!(
        Client::connect(addr, tablet, "wrong"),
        Err(ClientError::Refused(Refusal::NotPaired))
    ));

    let (mut a, _) = Client::connect(addr, tablet, "t-token")?;
    let (mut b, _) = Client::connect(addr, register2, "r-token")?;
    assert_eq!(a.register, MAIN_REGISTER);
    assert_eq!(b.register, second);

    // Each register opens its own shift.
    let shift_a = Uuid::new_v4();
    let shift_b = Uuid::new_v4();
    a.call(Uuid::new_v4(), Request::OpenShift(shift_a, "100".parse()?))?;
    b.call(Uuid::new_v4(), Request::OpenShift(shift_b, "50".parse()?))?;

    let bill = |qty| {
        let mut c = Cart::new();
        c.lines.push(CartLine {
            product_id: product.id,
            quantity: qty,
            ..Default::default()
        });
        c
    };
    let cash = |amount: &str| -> Result<PaymentRequest> {
        Ok(PaymentRequest {
            method: PaymentMethod::Cash,
            amount: amount.parse()?,
        })
    };
    let pay_a = Request::Pay(Box::new(bill(2)), cash("20")?, shift_a);
    let call_id = Uuid::new_v4();
    let Reply::Paid(first) = a.call(call_id, pay_a.clone())? else {
        return Err("expected Paid".into());
    };
    // The answer was "lost": the device reconnects and resends.
    drop(a);
    let (mut a, _) = Client::connect(addr, tablet, "t-token")?;
    let Reply::Paid(again) = a.call(call_id, pay_a)? else {
        return Err("expected Paid".into());
    };
    assert_eq!(first.number, again.number);

    // Only one unit is left: the second register cannot sell two.
    let short = b.call(
        Uuid::new_v4(),
        Request::Pay(Box::new(bill(2)), cash("20")?, shift_b),
    );
    assert!(matches!(short, Err(ClientError::Business(_))));
    // Paying against the other register's shift is refused.
    assert!(
        b.call(
            Uuid::new_v4(),
            Request::Pay(Box::new(bill(1)), cash("10")?, shift_a),
        )
        .is_err()
    );
    let Reply::Paid(_) = b.call(
        Uuid::new_v4(),
        Request::Pay(Box::new(bill(1)), cash("10")?, shift_b),
    )?
    else {
        return Err("expected Paid".into());
    };
    // B heard about A's sale while it waited.
    assert!(b.changes().iter().any(|v| v.get(Topic::Sales) > 0));

    // Each register sees its own drawer.
    let refresh = |c: &mut Client| -> Result<pos_service::Data> {
        match c.call(
            Uuid::new_v4(),
            Request::Refresh(Box::new(pos_service::Query {
                products: Default::default(),
                sale_offset: 0,
                report: pos_service::Period::day(pos_service::bangkok_date(1_760_000_000)),
                shift_offset: 0,
                movement_shift: None,
                movement_offset: 0,
                stock_product: None,
            })),
        )? {
            Reply::Data(d) => Ok(*d),
            _ => Err("expected Data".into()),
        }
    };
    let da = refresh(&mut a)?;
    let db = refresh(&mut b)?;
    assert_eq!(
        da.shift.as_ref().ok_or("a")?.expected()?.to_string(),
        "120.00"
    );
    assert_eq!(
        db.shift.as_ref().ok_or("b")?.expected()?.to_string(),
        "60.00"
    );
    assert_eq!(da.products[0].stock, Some(0));
    assert_eq!(da.sales.len(), 2);

    drop(a);
    drop(b);
    drop(hub);
    let _pos = owner.join().map_err(|_| "owner panicked")?;
    Ok(())
}

#[test]
fn sign_in_belongs_to_the_device() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("shop.sqlite3"))?;
    let owner_member = pos.save_staff(
        &pos_domain::StaffInput {
            name: "เจ้าของ".into(),
            role: Some("owner".into()),
            pin: Some("1234".into()),
            active: true,
            ..Default::default()
        },
        1,
    )?;
    pos.sign_out();
    let (submit, owner) = owner(pos);
    let d1 = Uuid::new_v4();
    let d2 = Uuid::new_v4();
    let authorize: pos_hub::Authorize =
        Arc::new(move |device, _| (device == d1 || device == d2).then_some(MAIN_REGISTER));
    let hub = Hub::start(TcpListener::bind("127.0.0.1:0")?, authorize, submit)?;
    let (mut one, _) = Client::connect(hub.address(), d1, "")?;
    let (mut two, _) = Client::connect(hub.address(), d2, "")?;
    one.call(
        Uuid::new_v4(),
        Request::SignIn(owner_member.id, "1234".into()),
    )?;
    // Device two is not signed in: a staffed shop requires sign-in.
    assert!(
        two.call(
            Uuid::new_v4(),
            Request::OpenShift(Uuid::new_v4(), "0".parse()?)
        )
        .is_err()
    );
    one.call(
        Uuid::new_v4(),
        Request::OpenShift(Uuid::new_v4(), "0".parse()?),
    )?;
    drop(one);
    drop(two);
    drop(hub);
    let _ = owner.join().map_err(|_| "owner panicked")?;
    Ok(())
}
