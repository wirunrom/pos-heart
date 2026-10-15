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
