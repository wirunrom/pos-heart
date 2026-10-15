//! A hub and devices over loopback TLS: pairing, pinning, two registers
//! selling the shared stock, per-device sign-in, retries after a dropped
//! connection, revocation and refusals.
use pos_core::{DeviceKind, MAIN_REGISTER, Pos};
use pos_domain::{Cart, CartLine, PaymentMethod, PaymentRequest, ProductInput};
use pos_hub::{Client, ClientError, Executor, Hub, Job, tls::Identity, wire::Refusal};
use pos_service::{Reply, Request, Topic};
use std::{
    net::TcpListener,
    sync::{Arc, Mutex, mpsc},
    thread,
};
use uuid::Uuid;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

type Task = Box<dyn FnOnce(&mut Pos, &mut Executor) + Send>;
enum Owner {
    Job(Job),
    /// Test hook: run something on the database owner.
    Do(Task),
}
/// The database owner: one thread running jobs in order, like the worker.
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
                Owner::Job(job) => executor.run(&mut pos, job, now),
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
fn query() -> pos_service::Query {
    pos_service::Query {
        products: Default::default(),
        sale_offset: 0,
        report: pos_service::Period::day(pos_service::bangkok_date(1_760_000_000)),
        shift_offset: 0,
        movement_shift: None,
        movement_offset: 0,
        stock_product: None,
    }
}

#[test]
fn paired_registers_sell_shared_stock_over_tls() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("shop.sqlite3"))?;
    let product = pos.save_product(ProductInput {
        sku: "A".into(),
        name: "น้ำดื่ม".into(),
        price: "10".parse()?,
        stock: Some(3),
        ..Default::default()
    })?;
    let (submit, control, owner) = owner(pos);
    let identity = Identity::generate()?;
    let hub = Hub::start(TcpListener::bind("127.0.0.1:0")?, &identity, submit)?;
    let addr = hub.address();

    // No pairing open, then a wrong code: refused.
    let tablet = Uuid::new_v4();
    assert!(matches!(
        Client::pair(addr, "000000", tablet, "iPad 1", DeviceKind::Ordering),
        Err(ClientError::Refused(Refusal::WrongCode))
    ));
    let code = hub.open_pairing();
    let wrong = if code == "111111" { "222222" } else { "111111" };
    assert!(matches!(
        Client::pair(addr, wrong, tablet, "iPad 1", DeviceKind::Ordering),
        Err(ClientError::Refused(Refusal::WrongCode))
    ));
    let a_pair = Client::pair(addr, &code, tablet, "iPad 1", DeviceKind::Ordering)?;
    assert_eq!(a_pair.register, MAIN_REGISTER);
    assert_eq!(a_pair.fingerprint, identity.fingerprint());
    // A code works once.
    assert!(Client::pair(addr, &code, Uuid::new_v4(), "x", DeviceKind::Ordering).is_err());
    let counter2 = Uuid::new_v4();
    let code = hub.open_pairing();
    let b_pair = Client::pair(addr, &code, counter2, "เคาน์เตอร์ 2", DeviceKind::Register)?;
    assert_eq!(
        b_pair.register, counter2,
        "a register device gets its own register"
    );

    // Unknown devices, wrong tokens and another certificate are refused.
    assert!(matches!(
        Client::connect(addr, Uuid::new_v4(), &a_pair.token, a_pair.fingerprint),
        Err(ClientError::Refused(Refusal::NotPaired))
    ));
    assert!(matches!(
        Client::connect(addr, tablet, "wrong", a_pair.fingerprint),
        Err(ClientError::Refused(Refusal::NotPaired))
    ));
    assert!(Client::connect(addr, tablet, &a_pair.token, [0u8; 32]).is_err());

    let (mut a, _) = Client::connect(addr, tablet, &a_pair.token, a_pair.fingerprint)?;
    let (mut b, _) = Client::connect(addr, counter2, &b_pair.token, b_pair.fingerprint)?;
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
    let (mut a, _) = Client::connect(addr, tablet, &a_pair.token, a_pair.fingerprint)?;
    let Reply::Paid(again) = a.call(call_id, pay_a)? else {
        return Err("expected Paid".into());
    };
    assert_eq!(first.number, again.number);

    // One unit left: the second register cannot sell two, nor use A's shift.
    assert!(matches!(
        b.call(
            Uuid::new_v4(),
            Request::Pay(Box::new(bill(2)), cash("20")?, shift_b)
        ),
        Err(ClientError::Business(_))
    ));
    assert!(
        b.call(
            Uuid::new_v4(),
            Request::Pay(Box::new(bill(1)), cash("10")?, shift_a)
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
    // A hears about B's sale without asking.
    let mut heard = false;
    for _ in 0..20 {
        if a.changes()?.iter().any(|v| v.get(Topic::Sales) >= 2) {
            heard = true;
            break;
        }
        thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(heard, "change notice reached the other device");

    let data = |c: &mut Client| -> Result<pos_service::Data> {
        match c.call(Uuid::new_v4(), Request::Refresh(Box::new(query())))? {
            Reply::Data(d) => Ok(*d),
            _ => Err("expected Data".into()),
        }
    };
    let da = data(&mut a)?;
    let db = data(&mut b)?;
    assert_eq!(
        da.shift.as_ref().ok_or("a")?.expected()?.to_string(),
        "120.00"
    );
    assert_eq!(
        db.shift.as_ref().ok_or("b")?.expected()?.to_string(),
        "60.00"
    );
    assert_eq!(da.products[0].stock, Some(0));
    assert_eq!(da.registers.len(), 2);

    // Revoking the tablet ends its access.
    let (done_tx, done_rx) = mpsc::channel();
    control.send(Owner::Do(Box::new(move |pos, executor| {
        let _ = done_tx.send(pos.revoke_device(tablet, 2_000_000_000));
        executor.forget(tablet);
    })))?;
    done_rx.recv()??;
    hub.disconnect(tablet);
    assert!(matches!(
        Client::connect(addr, tablet, &a_pair.token, a_pair.fingerprint),
        Err(ClientError::Refused(Refusal::NotPaired))
    ));

    drop(a);
    drop(b);
    drop(hub);
    drop(control);
    let mut pos = owner.join().map_err(|_| "owner panicked")?;
    let devices = pos.devices()?;
    assert_eq!(devices.len(), 2);
    assert!(
        devices
            .iter()
            .any(|d| d.id == tablet && d.revoked_at.is_some())
    );
    Ok(())
}

#[test]
fn sign_in_belongs_to_the_device() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("shop.sqlite3"))?;
    let member = pos.save_staff(
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
    let (submit, _control, owner) = owner(pos);
    let identity = Identity::generate()?;
    let hub = Hub::start(TcpListener::bind("127.0.0.1:0")?, &identity, submit)?;
    let d1 = Uuid::new_v4();
    let d2 = Uuid::new_v4();
    let p1 = Client::pair(
        hub.address(),
        &hub.open_pairing(),
        d1,
        "one",
        DeviceKind::Ordering,
    )?;
    let p2 = Client::pair(
        hub.address(),
        &hub.open_pairing(),
        d2,
        "two",
        DeviceKind::Ordering,
    )?;
    let (mut one, _) = Client::connect(hub.address(), d1, &p1.token, p1.fingerprint)?;
    let (mut two, _) = Client::connect(hub.address(), d2, &p2.token, p2.fingerprint)?;
    one.call(Uuid::new_v4(), Request::SignIn(member.id, "1234".into()))?;
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
    drop(_control);
    let _ = owner.join().map_err(|_| "owner panicked")?;
    Ok(())
}
