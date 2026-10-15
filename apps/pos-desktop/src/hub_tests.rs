//! The desktop worker as the shop hub: a paired device's requests run on
//! the worker's database, the counter's own writes reach the device as
//! change notices, and revoking ends the device's access.
use crate::worker::{Command, Event, Worker};
use pos_core::DeviceKind;
use pos_hub::{Client, ClientError, Hub, tls::Identity, wire::Refusal};
use pos_service::{Reply, Request, Topic};
use std::{net::TcpListener, time::Duration};
use uuid::Uuid;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn wait_for(worker: &Worker, mut want: impl FnMut(&Event) -> bool) -> Result {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        if let Ok(e) = worker.rx.recv_timeout(Duration::from_millis(100))
            && want(&e.event)
        {
            return Ok(());
        }
    }
    Err("timed out waiting for the worker".into())
}

#[test]
fn a_paired_device_works_through_the_desktop_worker() -> Result {
    let dir = tempfile::tempdir()?;
    let worker = Worker::start(
        dir.path().join("pos-heart.sqlite3"),
        eframe::egui::Context::default(),
    )?;
    wait_for(&worker, |e| matches!(e, Event::Ready(_)))?;
    let identity = Identity::load_or_create(dir.path())?;
    let hub = Hub::start(
        TcpListener::bind("127.0.0.1:0")?,
        &identity,
        worker.hub_submit(),
    )?;
    assert!(worker.send(Command::SetHub(Some(hub.notifier()))));
    let device = Uuid::new_v4();
    let paired = Client::pair(
        hub.address(),
        &hub.open_pairing(),
        device,
        "iPad โต๊ะ",
        DeviceKind::Ordering,
    )?;
    let (mut client, _) =
        Client::connect(hub.address(), device, &paired.token, paired.fingerprint)?;
    // The device adds a product; the counter hears that devices changed data.
    let Reply::ProductSaved(product) = client.call(
        Uuid::new_v4(),
        Request::SaveProduct(Box::new(pos_domain::ProductInput {
            sku: "T-1".into(),
            name: "ชาไทย".into(),
            price: "45".parse()?,
            ..Default::default()
        })),
    )?
    else {
        return Err("expected ProductSaved".into());
    };
    wait_for(&worker, |e| matches!(e, Event::DevicesChanged))?;
    // A write on the counter reaches the device as a change notice.
    assert!(worker.send(Command::Archive(product.id)));
    wait_for(&worker, |e| matches!(e, Event::Archived))?;
    let mut heard = false;
    for _ in 0..40 {
        if client.changes()?.iter().any(|v| v.get(Topic::Catalog) >= 2) {
            heard = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(heard, "the device heard about the counter's change");
    // Revoking from the counter ends the device's access.
    assert!(worker.send(Command::RevokeDevice(device)));
    wait_for(&worker, |e| matches!(e, Event::DeviceRevoked(_)))?;
    assert!(matches!(
        Client::connect(hub.address(), device, &paired.token, paired.fingerprint),
        Err(ClientError::Refused(Refusal::NotPaired))
    ));
    Ok(())
}

#[test]
fn a_client_mode_worker_runs_commands_on_the_main_register() -> Result {
    use crate::worker::DeviceFile;
    let dir = tempfile::tempdir()?;
    let main = Worker::start(
        dir.path().join("pos-heart.sqlite3"),
        eframe::egui::Context::default(),
    )?;
    wait_for(&main, |e| matches!(e, Event::Ready(_)))?;
    let identity = Identity::load_or_create(dir.path())?;
    let hub = Hub::start(
        TcpListener::bind("127.0.0.1:0")?,
        &identity,
        main.hub_submit(),
    )?;
    assert!(main.send(Command::SetHub(Some(hub.notifier()))));
    let device = Uuid::new_v4();
    let paired = Client::pair(
        hub.address(),
        &hub.open_pairing(),
        device,
        "เคาน์เตอร์ 2",
        DeviceKind::Register,
    )?;
    let file = DeviceFile {
        hub: hub.address(),
        device,
        token: paired.token,
        fingerprint: DeviceFile::hex(&paired.fingerprint),
        name: "เคาน์เตอร์ 2".into(),
        kind: DeviceKind::Register,
        register: paired.register,
    };
    let client_dir = tempfile::tempdir()?;
    file.save(client_dir.path())?;
    let loaded = DeviceFile::load(client_dir.path()).ok_or("device.json")?;
    assert_eq!(loaded.pin(), Some(paired.fingerprint));
    let remote = Worker::start_remote(
        loaded,
        client_dir.path().to_path_buf(),
        eframe::egui::Context::default(),
    )?;
    wait_for(&remote, |e| matches!(e, Event::Connection(true)))?;
    // A product saved on the device lands in the main register's database.
    assert!(remote.send(Command::Save(pos_domain::ProductInput {
        sku: "R-1".into(),
        name: "กาแฟ".into(),
        price: "50".parse()?,
        ..Default::default()
    })));
    wait_for(&remote, |e| matches!(e, Event::Saved(_)))?;
    wait_for(&main, |e| matches!(e, Event::DevicesChanged))?;
    // The device's own register has its own shift.
    assert!(remote.send(Command::OpenShift(Uuid::new_v4(), "200".parse()?)));
    wait_for(&remote, |e| matches!(e, Event::ShiftOpened))?;
    // Business errors come back as the same local errors.
    assert!(remote.send(Command::OpenShift(Uuid::new_v4(), "1".parse()?)));
    wait_for(&remote, |e| {
        matches!(e, Event::Failed(pos_core::Error::ShiftAlreadyOpen))
    })?;
    // Device-local work is refused on a device.
    assert!(remote.send(Command::Demo));
    wait_for(&remote, |e| {
        matches!(e, Event::Failed(pos_core::Error::Remote(_)))
    })?;
    Ok(())
}

#[test]
fn the_counter_hears_when_a_device_changed_its_open_bill() -> Result {
    let dir = tempfile::tempdir()?;
    let worker = Worker::start(
        dir.path().join("pos-heart.sqlite3"),
        eframe::egui::Context::default(),
    )?;
    wait_for(&worker, |e| matches!(e, Event::Ready(_)))?;
    let identity = Identity::load_or_create(dir.path())?;
    let hub = Hub::start(
        TcpListener::bind("127.0.0.1:0")?,
        &identity,
        worker.hub_submit(),
    )?;
    assert!(worker.send(Command::SetHub(Some(hub.notifier()))));
    let device = Uuid::new_v4();
    let paired = Client::pair(
        hub.address(),
        &hub.open_pairing(),
        device,
        "iPad",
        DeviceKind::Ordering,
    )?;
    let (mut client, _) =
        Client::connect(hub.address(), device, &paired.token, paired.fingerprint)?;
    assert!(worker.send(Command::Save(pos_domain::ProductInput {
        sku: "K-1".into(),
        name: "ข้าวผัด".into(),
        price: "60".parse()?,
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
    let mut counter = pos_domain::Cart::new();
    counter.lines.push(pos_domain::CartLine {
        product_id: product.ok_or("product")?,
        quantity: 1,
        ..Default::default()
    });
    // The counter autosaves its bill; the device loads it and changes it.
    assert!(worker.send(Command::Autosave(counter.clone())));
    wait_for(&worker, |e| matches!(e, Event::Quiet))?;
    let Reply::Resumed(mut on_device, _) =
        client.call(Uuid::new_v4(), Request::Resume(counter.id))?
    else {
        return Err("expected Resumed".into());
    };
    on_device.lines[0].quantity = 4;
    client.call(Uuid::new_v4(), Request::Hold(on_device))?;
    // The counter's next autosave of its old copy is refused and reported,
    // as is an explicit hold.
    counter.lines[0].quantity = 2;
    assert!(worker.send(Command::Autosave(counter.clone())));
    wait_for(&worker, |e| matches!(e, Event::BillChanged))?;
    assert!(worker.send(Command::Hold(counter)));
    wait_for(&worker, |e| {
        matches!(e, Event::Failed(pos_core::Error::BillChanged))
    })?;
    Ok(())
}
