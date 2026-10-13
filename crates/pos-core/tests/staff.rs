use pos_core::{Error, Pos};
use pos_domain::{
    Cart, CartLine, Discount, Money, PaymentMethod, PaymentRequest, Permission, Product,
    ProductInput, RefundLine, RefundRequest, Staff, StaffInput, StockKind, StockRequest,
};
use std::error::Error as StdError;
use time::macros::{date, offset};
use uuid::Uuid;

type Result<T = ()> = std::result::Result<T, Box<dyn StdError>>;
fn member(name: &str, role: &str, pin: &str) -> StaffInput {
    StaffInput {
        id: None,
        name: name.into(),
        role: Some(role.into()),
        pin: Some(pin.into()),
        active: true,
    }
}
struct Shop {
    pos: Pos,
    owner: Staff,
    cashier: Staff,
    manager: Staff,
    shift: Uuid,
    water: Product,
    _dir: tempfile::TempDir,
}
/// Products and a shift exist before staff, then staff is switched on.
fn shop() -> Result<Shop> {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let water = pos.save_product(ProductInput {
        sku: "W".into(),
        name: "น้ำ".into(),
        price: "10".parse()?,
        stock: Some(50),
        ..Default::default()
    })?;
    let owner = pos.save_staff(&member("เจ้าของ", pos_domain::OWNER, "9999"), 1)?;
    let manager = pos.save_staff(&member("ผู้จัดการ", pos_domain::MANAGER, "5555"), 1)?;
    let cashier = pos.save_staff(&member("นิด", pos_domain::CASHIER, "1234"), 1)?;
    let shift = Uuid::new_v4();
    pos.sign_in(cashier.id, "1234")?;
    pos.open_shift(shift, "100".parse()?, 2)?;
    Ok(Shop {
        pos,
        owner,
        cashier,
        manager,
        shift,
        water,
        _dir: dir,
    })
}
fn cart(s: &Shop, discount: Option<Discount>) -> Cart {
    let mut cart = Cart::new();
    cart.lines.push(CartLine {
        product_id: s.water.id,
        quantity: 10,
        ..Default::default()
    });
    cart.discount = discount;
    cart
}
fn pay(s: &mut Shop, discount: Option<Discount>, at: i64) -> pos_core::Result<pos_domain::Sale> {
    let cart = cart(s, discount);
    s.pos.checkout_with_payment(
        &cart,
        PaymentRequest {
            method: PaymentMethod::Cash,
            amount: Money::from_satang(10_000)?,
        },
        Some(s.shift),
        at,
    )
}

#[test]
fn first_staff_must_be_an_owner_and_shops_without_staff_need_no_sign_in() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    assert!(!pos.staff_enabled()?);
    pos.open_shift(Uuid::new_v4(), Money::ZERO, 1)?;
    assert!(matches!(
        pos.save_staff(&member("นิด", pos_domain::CASHIER, "1234"), 1),
        Err(Error::LastOwner)
    ));
    let owner = pos.save_staff(&member("เจ้าของ", pos_domain::OWNER, "9999"), 1)?;
    // The first owner is signed in to keep setting up.
    assert_eq!(pos.signed_in()?.map(|s| s.id), Some(owner.id));
    pos.sign_out();
    assert!(matches!(
        pos.save_product(ProductInput {
            sku: "A".into(),
            name: "a".into(),
            ..Default::default()
        }),
        Err(Error::SignInRequired)
    ));
    // The last active owner cannot be demoted or switched off.
    pos.sign_in(owner.id, "9999")?;
    let demote = StaffInput {
        id: Some(owner.id),
        pin: None,
        ..member("เจ้าของ", pos_domain::MANAGER, "0000")
    };
    assert!(matches!(pos.save_staff(&demote, 2), Err(Error::LastOwner)));
    Ok(())
}

#[test]
fn sales_record_who_rang_them_up_and_cashiers_need_approval() -> Result {
    let mut s = shop()?;
    let sale = pay(&mut s, None, 10)?;
    assert_eq!(sale.staff.by.as_ref().map(|b| b.id), Some(s.cashier.id));
    assert!(sale.staff.approved_by.is_none());
    // 10% is within the default cashier limit; 15% is not.
    pay(&mut s, Some(Discount::Percent(1_000)), 11)?;
    let big = Some(Discount::Percent(1_500));
    assert!(matches!(
        pay(&mut s, big, 12),
        Err(Error::ApprovalRequired(Permission::Discount))
    ));
    assert_eq!(s.pos.product(s.water.id)?.stock, Some(30));
    // A cashier cannot approve; a manager can, once.
    assert!(matches!(
        s.pos.approve(s.cashier.id, "1234", Permission::Discount),
        Err(Error::ApprovalRequired(_))
    ));
    s.pos.approve(s.manager.id, "5555", Permission::Discount)?;
    let approved = pay(&mut s, big, 13)?;
    assert_eq!(approved.staff.approved_by.map(|a| a.id), Some(s.manager.id));
    s.pos.clear_approval();
    // Refunds need a manager too.
    let request = RefundRequest {
        id: Uuid::new_v4(),
        sale_id: sale.id,
        refund_method: PaymentMethod::Cash,
        reason: "คืน".into(),
        lines: vec![RefundLine {
            line: 0,
            quantity: 1,
        }],
        legacy_restock: None,
    };
    assert!(matches!(
        s.pos.refund_items(&request, Some(s.shift), 20),
        Err(Error::ApprovalRequired(Permission::Refund))
    ));
    s.pos.approve(s.manager.id, "5555", Permission::Refund)?;
    let refund = s.pos.refund_items(&request, Some(s.shift), 21)?;
    assert_eq!(refund.staff.approved_by.map(|a| a.id), Some(s.manager.id));
    s.pos.clear_approval();
    // Stock changes and drawer movements are restricted as well.
    let count = StockRequest {
        id: Uuid::new_v4(),
        product_id: s.water.id,
        kind: StockKind::Receive,
        quantity: 5,
        expected: None,
        reason: String::new(),
    };
    assert!(matches!(
        s.pos.adjust_stock(&count, 22),
        Err(Error::ApprovalRequired(Permission::Stock))
    ));
    s.pos.sign_in(s.manager.id, "5555")?;
    let moved = s.pos.adjust_stock(&count, 23)?;
    assert_eq!(moved.staff.by.map(|b| b.id), Some(s.manager.id));
    let logged = s.pos.stock_movements(s.water.id, 0)?;
    let logged = logged.iter().find(|m| m.id == moved.id).ok_or("movement")?;
    assert_eq!(
        logged.staff.by.as_ref().map(|b| b.role.as_str()),
        Some(pos_domain::MANAGER)
    );
    Ok(())
}

#[test]
fn wrong_pins_lock_and_inactive_staff_cannot_act() -> Result {
    let mut s = shop()?;
    for _ in 0..4 {
        assert!(matches!(
            s.pos.sign_in(s.manager.id, "0000"),
            Err(Error::WrongPin)
        ));
    }
    assert!(matches!(
        s.pos.sign_in(s.manager.id, "0000"),
        Err(Error::WrongPin)
    ));
    // Locked even with the right PIN now.
    assert!(matches!(
        s.pos.sign_in(s.manager.id, "5555"),
        Err(Error::PinLocked(_))
    ));
    // The cashier is still signed in; an owner switches them off.
    let mut s2 = shop()?;
    s2.pos.sign_in(s2.owner.id, "9999")?;
    let off = StaffInput {
        id: Some(s2.cashier.id),
        pin: None,
        active: false,
        ..member("นิด", pos_domain::CASHIER, "0000")
    };
    s2.pos.save_staff(&off, 3)?;
    s2.pos.sign_out();
    assert!(matches!(
        s2.pos.sign_in(s2.cashier.id, "1234"),
        Err(Error::StaffNotFound)
    ));
    // Changing a PIN keeps the new one only.
    let repin = StaffInput {
        id: Some(s2.manager.id),
        pin: Some("2468".into()),
        ..member("ผู้จัดการ", pos_domain::MANAGER, "0000")
    };
    s2.pos.sign_in(s2.owner.id, "9999")?;
    s2.pos.save_staff(&repin, 4)?;
    assert!(matches!(
        s2.pos.sign_in(s2.manager.id, "5555"),
        Err(Error::WrongPin)
    ));
    s2.pos.sign_in(s2.manager.id, "2468")?;
    // Only owners manage staff and the discount limit.
    assert!(matches!(
        s2.pos.save_role(&pos_domain::RoleDef::cashier(5_000)),
        Err(Error::Forbidden(Permission::ManageStaff))
    ));
    Ok(())
}

#[test]
fn shifts_and_reports_name_the_staff() -> Result {
    let mut s = shop()?;
    let day = date!(2026 - 10 - 13);
    let at = day.midnight().assume_offset(offset!(+7)).unix_timestamp() + 3600;
    let shift = s.pos.active_shift()?.ok_or("shift")?;
    assert_eq!(shift.opened_by.map(|b| b.id), Some(s.cashier.id));
    pay(&mut s, None, at)?;
    s.pos.sign_in(s.manager.id, "5555")?;
    pay(&mut s, None, at + 60)?;
    pay(&mut s, None, at + 120)?;
    let r = s.pos.daily_summary(day)?;
    assert_eq!(r.staff.len(), 2);
    assert_eq!(r.staff[0].staff_id, s.manager.id);
    assert_eq!(
        (r.staff[0].sale_count, r.staff[0].sales),
        (2, "200".parse()?)
    );
    let shift = s.pos.active_shift()?.ok_or("shift")?;
    let counted = Money::from_satang(shift.expected()?.satang())?;
    let closed = s
        .pos
        .close_shift(s.shift, shift.revision, counted, at + 200)?;
    assert_eq!(
        closed.closing.and_then(|c| c.closed_by).map(|b| b.name),
        Some("ผู้จัดการ".into())
    );
    Ok(())
}

#[test]
fn custom_roles_allow_deny_or_need_approval() -> Result {
    use pos_domain::{Access, RoleDef};
    let mut s = shop()?;
    s.pos.sign_in(s.owner.id, "9999")?;
    // A shift lead refunds alone, may approve refunds, but never moves cash.
    let mut lead = RoleDef::cashier(2_000);
    lead.id = String::new();
    lead.name = "หัวหน้ากะ".into();
    lead.can_approve = true;
    lead.access.insert(Permission::Refund, Access::Allow);
    lead.access.insert(Permission::CashMovement, Access::Deny);
    let lead = s.pos.save_role(&lead)?;
    assert_eq!(s.pos.roles()?.len(), 4);
    let ple = s.pos.save_staff(&member("เปิ้ล", &lead.id, "7777"), 2)?;
    s.pos.sign_in(ple.id, "7777")?;
    // 15% is within this role's 20% limit.
    let sale = pay(&mut s, Some(Discount::Percent(1_500)), 10)?;
    let refund = RefundRequest {
        id: Uuid::new_v4(),
        sale_id: sale.id,
        refund_method: PaymentMethod::Cash,
        reason: "คืน".into(),
        lines: vec![RefundLine {
            line: 0,
            quantity: 1,
        }],
        legacy_restock: None,
    };
    let done = s.pos.refund_items(&refund, Some(s.shift), 11)?;
    assert_eq!(done.staff.by.map(|b| b.role_name), Some("หัวหน้ากะ".into()));
    let cash = pos_domain::CashMovement {
        id: Uuid::new_v4(),
        shift_id: s.shift,
        created_at: 12,
        kind: pos_domain::CashKind::CashOut,
        amount: "10".parse()?,
        reason: "ซื้อน้ำแข็ง".into(),
        payment_method: PaymentMethod::Cash,
        staff: Default::default(),
    };
    assert!(matches!(
        s.pos.move_cash(&cash),
        Err(Error::Forbidden(Permission::CashMovement))
    ));
    // Even an owner's approval does not override a denial.
    s.pos
        .approve(s.owner.id, "9999", Permission::CashMovement)?;
    assert!(matches!(
        s.pos.move_cash(&cash),
        Err(Error::Forbidden(Permission::CashMovement))
    ));
    s.pos.clear_approval();
    // The lead may approve a cashier's refund; the cashier cannot approve themselves.
    s.pos.sign_in(s.cashier.id, "1234")?;
    assert!(
        s.pos
            .approve(s.cashier.id, "1234", Permission::Refund)
            .is_err()
    );
    s.pos.approve(ple.id, "7777", Permission::Refund)?;
    let again = RefundRequest {
        id: Uuid::new_v4(),
        ..refund
    };
    let approved = s.pos.refund_items(&again, Some(s.shift), 13)?;
    assert_eq!(approved.staff.approved_by.map(|a| a.id), Some(ple.id));
    s.pos.clear_approval();
    // Roles in use cannot be deleted; built-ins never.
    s.pos.sign_in(s.owner.id, "9999")?;
    assert!(matches!(s.pos.delete_role(&lead.id), Err(Error::RoleInUse)));
    assert!(s.pos.delete_role(pos_domain::CASHIER).is_err());
    Ok(())
}
