//! Session-generation regression tests use real worker commands against a disposable database.
use super::*;
use pos_domain::{OWNER, ProductInput, StaffInput};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn owner(pos: &mut Pos) -> Result<Uuid> {
    Ok(pos
        .save_staff(
            &StaffInput {
                id: None,
                name: "Owner".into(),
                role: Some(OWNER.into()),
                pin: Some("1234".into()),
                active: true,
            },
            0,
        )?
        .id)
}
#[test]
fn queued_mutation_signin_and_approval_from_revoked_epoch_refuse_but_cart_saves_survive() -> Result
{
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let id = owner(&mut pos)?;
    pos.sign_in(id, "1234")?;
    let product = pos.save_product(ProductInput {
        sku: "A".into(),
        name: "A".into(),
        price: "10".parse()?,
        ..Default::default()
    })?;
    let epoch = AtomicU64::new(1);
    let mut observed = 0;
    let edit = ProductInput {
        id: Some(product.id),
        sku: "A".into(),
        name: "CHANGED".into(),
        price: "10".parse()?,
        ..Default::default()
    };
    for command in [
        Command::Save(edit.clone()),
        Command::SignIn(id, "1234".into()),
        Command::Approved {
            approver: id,
            pin: "1234".into(),
            permission: pos_domain::Permission::Catalog,
            inner: Box::new(Command::Save(edit)),
        },
    ] {
        let result = execute(
            &mut pos,
            CommandEnvelope {
                generation: 0,
                command,
            },
            &epoch,
            &mut observed,
        );
        assert!(matches!(result.event, Event::Failed(Error::SignInRequired)));
        assert_eq!(result.generation, 0);
    }
    assert_eq!(pos.product(product.id)?.name, "A");
    let mut cart = Cart::new();
    cart.lines.push(pos_domain::CartLine {
        product_id: product.id,
        quantity: 1,
        ..Default::default()
    });
    let result = execute(
        &mut pos,
        CommandEnvelope {
            generation: 0,
            command: Command::Autosave(cart.clone()),
        },
        &epoch,
        &mut observed,
    );
    assert!(matches!(result.event, Event::Quiet));
    assert_eq!(pos.held()?.first().map(|c| c.id), Some(cart.id));
    let result = execute(
        &mut pos,
        CommandEnvelope {
            generation: 0,
            command: Command::Restore(cart.id),
        },
        &epoch,
        &mut observed,
    );
    assert!(matches!(result.event, Event::Restored(..)));
    let result = execute(
        &mut pos,
        CommandEnvelope {
            generation: 0,
            command: Command::TicketLine(Uuid::new_v4(), 0, true),
        },
        &epoch,
        &mut observed,
    );
    assert!(matches!(result.event, Event::Quiet));
    let result = execute(
        &mut pos,
        CommandEnvelope {
            generation: 1,
            command: Command::SignIn(id, "1234".into()),
        },
        &epoch,
        &mut observed,
    );
    assert!(matches!(result.event, Event::SignedIn(_)));
    assert_eq!(result.generation, 1);
    Ok(())
}
#[test]
fn revoke_is_nonblocking_even_with_a_full_busy_queue() -> Result {
    let (tx, _commands) = mpsc::sync_channel(1);
    let (_events, rx) = mpsc::channel();
    let worker = Worker {
        tx,
        rx,
        epoch: Arc::new(AtomicU64::new(0)),
    };
    assert!(worker.send(Command::SignIn(Uuid::new_v4(), "1234".into())));
    worker.revoke(); // The wake command cannot fit; the atomic revocation still happens.
    assert_eq!(worker.generation(), 1);
    assert!(!worker.send(Command::SignIn(Uuid::new_v4(), "1234".into())));
    Ok(())
}

#[test]
fn a_completed_signin_reply_stays_stale_and_refresh_cannot_restore_its_identity() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let id = owner(&mut pos)?;
    pos.sign_out();
    let epoch = AtomicU64::new(0);
    let mut observed = 0;
    let reply = execute(
        &mut pos,
        CommandEnvelope {
            generation: 0,
            command: Command::SignIn(id, "1234".into()),
        },
        &epoch,
        &mut observed,
    );
    assert!(matches!(reply.event, Event::SignedIn(_)));
    epoch.fetch_add(1, Ordering::SeqCst); // Lock after completion, before the UI receives it.
    assert_ne!(reply.generation, epoch.load(Ordering::SeqCst));
    let query = Query {
        products: ProductQuery::default(),
        sale_offset: 0,
        report_start: today(),
        report_end: today(),
        shift_offset: 0,
        movement_shift: None,
        movement_offset: 0,
        stock_product: None,
    };
    let refresh = execute(
        &mut pos,
        CommandEnvelope {
            generation: 1,
            command: Command::Refresh(query),
        },
        &epoch,
        &mut observed,
    );
    let Event::Ready(data) = refresh.event else {
        return Err("expected refresh".into());
    };
    assert!(data.signed_in.is_none());
    Ok(())
}
