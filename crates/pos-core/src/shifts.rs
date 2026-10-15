use crate::{
    Error, Pos, Result,
    schema::{cash_movements, registers, shifts},
};
use diesel::prelude::*;
use pos_domain::{
    CashKind, CashMovement, DomainError, Money, Shift, ShiftClosing, ShiftCount, SignedMoney,
};
use uuid::Uuid;

pub(crate) fn active(
    conn: &mut SqliteConnection,
    register: Uuid,
    expected: Option<Uuid>,
    now: i64,
) -> Result<Shift> {
    let json = shifts::table
        .filter(shifts::closed_at.is_null())
        .filter(shifts::register_id.eq(register.to_string()))
        .select(shifts::shift_json)
        .first::<String>(conn)
        .optional()?
        .ok_or(Error::NoOpenShift)?;
    let shift: Shift = serde_json::from_str(&json)?;
    if expected.is_some_and(|id| id != shift.id) {
        return Err(Error::ShiftChanged);
    }
    if now < shift.opened_at {
        return Err(Error::InvalidShiftTime);
    }
    Ok(shift)
}
fn save(conn: &mut SqliteConnection, shift: &Shift) -> Result<()> {
    diesel::update(shifts::table.find(shift.id.to_string()))
        .set((
            shifts::closed_at.eq(shift.closing.as_ref().map(|c| c.closed_at)),
            shifts::shift_json.eq(serde_json::to_string(shift)?),
        ))
        .execute(conn)?;
    Ok(())
}
/// Caller owns the IMMEDIATE transaction, including sale/refund/stock writes.
pub(crate) fn record(
    conn: &mut SqliteConnection,
    shift: Shift,
    movement: CashMovement,
) -> Result<()> {
    record_all(conn, shift, vec![movement])
}
/// Several ledger rows of one sale or refund (the parts of a split payment):
/// each method's total moves, the sale/refund count only once.
pub(crate) fn record_all(
    conn: &mut SqliteConnection,
    mut shift: Shift,
    movements: Vec<CashMovement>,
) -> Result<()> {
    for (i, movement) in movements.iter().enumerate() {
        apply(&mut shift, movement, i == 0)?;
    }
    shift.expected()?;
    shift.revision = shift.revision.checked_add(1).ok_or(DomainError::Overflow)?;
    for movement in &movements {
        shift.last_activity_at = shift.last_activity_at.max(movement.created_at);
        diesel::insert_into(cash_movements::table)
            .values((
                cash_movements::id.eq(movement.id.to_string()),
                cash_movements::payment_method.eq(movement.payment_method.key()),
                cash_movements::shift_id.eq(shift.id.to_string()),
                cash_movements::created_at.eq(movement.created_at),
                cash_movements::movement_json.eq(serde_json::to_string(movement)?),
            ))
            .execute(conn)?;
    }
    save(conn, &shift)
}
fn apply(shift: &mut Shift, movement: &CashMovement, count: bool) -> Result<()> {
    match movement.kind {
        CashKind::Sale => {
            let total = match movement.payment_method {
                pos_domain::PaymentMethod::Cash => &mut shift.sales,
                pos_domain::PaymentMethod::Transfer => &mut shift.transfer_sales,
                pos_domain::PaymentMethod::Card => &mut shift.card_sales,
            };
            *total = total.checked_add(movement.amount)?;
            if count {
                shift.sale_count = shift
                    .sale_count
                    .checked_add(1)
                    .ok_or(DomainError::Overflow)?;
            }
        }
        CashKind::Refund => {
            let total = match movement.payment_method {
                pos_domain::PaymentMethod::Cash => &mut shift.refunds,
                pos_domain::PaymentMethod::Transfer => &mut shift.transfer_refunds,
                pos_domain::PaymentMethod::Card => &mut shift.card_refunds,
            };
            *total = total.checked_add(movement.amount)?;
            if count {
                shift.refund_count = shift
                    .refund_count
                    .checked_add(1)
                    .ok_or(DomainError::Overflow)?;
            }
        }
        CashKind::CashIn => shift.cash_in = shift.cash_in.checked_add(movement.amount)?,
        CashKind::CashOut => shift.cash_out = shift.cash_out.checked_add(movement.amount)?,
    }
    Ok(())
}
/// A register: one drawer and its own shift (multi-device phase 0c).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Register {
    pub id: Uuid,
    pub name: String,
    pub created_at: i64,
    pub retired_at: Option<i64>,
}
impl Pos {
    /// The register this connection sells for.
    pub fn register(&self) -> Uuid {
        self.register
    }
    /// Sells for another (existing, not retired) register from now on.
    pub fn use_register(&mut self, id: Uuid) -> Result<()> {
        let retired = registers::table
            .find(id.to_string())
            .select(registers::retired_at)
            .first::<Option<i64>>(&mut self.conn)
            .optional()?
            .ok_or(Error::RegisterNotFound)?;
        if retired.is_some() {
            return Err(Error::RegisterNotFound);
        }
        self.register = id;
        Ok(())
    }
    /// Every register, the main one first.
    pub fn registers(&mut self) -> Result<Vec<Register>> {
        registers::table
            .order((registers::created_at, registers::id))
            .select((
                registers::id,
                registers::name,
                registers::created_at,
                registers::retired_at,
            ))
            .load::<(String, String, i64, Option<i64>)>(&mut self.conn)?
            .into_iter()
            .map(|(id, name, created_at, retired_at)| {
                Ok(Register {
                    id: Uuid::parse_str(&id).map_err(|_| Error::CorruptData)?,
                    name,
                    created_at,
                    retired_at,
                })
            })
            .collect()
    }
    /// Adds a register (Settings permission). A retried request with the same
    /// ID and name returns the stored register.
    pub fn add_register(&mut self, id: Uuid, name: &str, now: i64) -> Result<Register> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 80 {
            return Err(Error::InvalidRegister);
        }
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            crate::staff::authorize(conn, &session, Some(pos_domain::Permission::Settings))?;
            if let Some(existing) = registers::table
                .find(id.to_string())
                .select(registers::name)
                .first::<String>(conn)
                .optional()?
            {
                return if existing == name {
                    Ok(Register {
                        id,
                        name: existing,
                        created_at: now,
                        retired_at: None,
                    })
                } else {
                    Err(Error::ConflictingRequest)
                };
            }
            diesel::insert_into(registers::table)
                .values((
                    registers::id.eq(id.to_string()),
                    registers::name.eq(name),
                    registers::created_at.eq(now),
                ))
                .execute(conn)?;
            Ok(Register {
                id,
                name: name.to_owned(),
                created_at: now,
                retired_at: None,
            })
        })
    }
    /// The open shift of this connection's register.
    pub fn active_shift(&mut self) -> Result<Option<Shift>> {
        shifts::table
            .filter(shifts::closed_at.is_null())
            .filter(shifts::register_id.eq(self.register.to_string()))
            .select(shifts::shift_json)
            .first::<String>(&mut self.conn)
            .optional()?
            .map(|s| Ok(serde_json::from_str(&s)?))
            .transpose()
    }
    pub fn shift_history(&mut self, offset: u32) -> Result<Vec<Shift>> {
        shifts::table
            .filter(shifts::closed_at.is_not_null())
            .order((shifts::closed_at.desc(), shifts::id.desc()))
            .limit(50)
            .offset(i64::from(offset))
            .select(shifts::shift_json)
            .load::<String>(&mut self.conn)?
            .into_iter()
            .map(|s| Ok(serde_json::from_str(&s)?))
            .collect()
    }
    /// Every shift's 1-based position in opening order, in one scan.
    pub fn shift_numbers(&mut self) -> Result<std::collections::HashMap<Uuid, u64>> {
        let ids = shifts::table
            .order((shifts::opened_at, shifts::id))
            .select(shifts::id)
            .load::<String>(&mut self.conn)?;
        ids.into_iter()
            .zip(1_u64..)
            .map(|(id, n)| Ok((Uuid::parse_str(&id).map_err(|_| Error::CorruptData)?, n)))
            .collect()
    }
    /// 1-based position of a shift in opening order (shown as "Shift #12").
    pub fn shift_number(&mut self, id: Uuid) -> Result<u64> {
        let (opened_at, key) = shifts::table
            .find(id.to_string())
            .select((shifts::opened_at, shifts::id))
            .first::<(i64, String)>(&mut self.conn)
            .optional()?
            .ok_or(Error::ShiftClosed)?;
        let before = shifts::table
            .filter(
                shifts::opened_at
                    .lt(opened_at)
                    .or(shifts::opened_at.eq(opened_at).and(shifts::id.le(key))),
            )
            .count()
            .get_result::<i64>(&mut self.conn)?;
        u64::try_from(before).map_err(|_| Error::CorruptData)
    }
    pub fn cash_movements(&mut self, id: Uuid, offset: u32) -> Result<Vec<CashMovement>> {
        cash_movements::table
            .filter(cash_movements::shift_id.eq(id.to_string()))
            .order((cash_movements::created_at.desc(), cash_movements::id.desc()))
            .limit(50)
            .offset(i64::from(offset))
            .select(cash_movements::movement_json)
            .load::<String>(&mut self.conn)?
            .into_iter()
            .map(|s| Ok(serde_json::from_str(&s)?))
            .collect()
    }
    /// Stable request ID makes a lost response safe to retry, including after closing.
    pub fn open_shift(&mut self, id: Uuid, opening: Money, now: i64) -> Result<Shift> {
        let session = self.session.clone();
        let register = self.register;
        self.conn.immediate_transaction(|conn| {
            if let Some(json) = shifts::table
                .find(id.to_string())
                .select(shifts::shift_json)
                .first::<String>(conn)
                .optional()?
            {
                let shift: Shift = serde_json::from_str(&json)?;
                return if shift.opening == opening {
                    Ok(shift)
                } else {
                    Err(Error::ConflictingRequest)
                };
            }
            if shifts::table
                .filter(shifts::closed_at.is_null())
                .filter(shifts::register_id.eq(register.to_string()))
                .count()
                .get_result::<i64>(conn)?
                != 0
            {
                return Err(Error::ShiftAlreadyOpen);
            }
            let last = shifts::table
                .filter(shifts::register_id.eq(register.to_string()))
                .select(diesel::dsl::max(shifts::closed_at))
                .first::<Option<i64>>(conn)?;
            if last.is_some_and(|closed| now < closed) {
                return Err(Error::InvalidShiftTime);
            }
            let staff = crate::staff::authorize(conn, &session, None)?;
            let shift = Shift {
                id,
                register_id: register,
                opened_at: now,
                last_activity_at: now,
                opening,
                sales: Money::ZERO,
                transfer_sales: Money::ZERO,
                card_sales: Money::ZERO,
                transfer_refunds: Money::ZERO,
                card_refunds: Money::ZERO,
                refunds: Money::ZERO,
                cash_in: Money::ZERO,
                cash_out: Money::ZERO,
                sale_count: 0,
                refund_count: 0,
                revision: 0,
                closing: None,
                opened_by: staff.by,
            };
            diesel::insert_into(shifts::table)
                .values((
                    shifts::id.eq(id.to_string()),
                    shifts::register_id.eq(register.to_string()),
                    shifts::opened_at.eq(now),
                    shifts::shift_json.eq(serde_json::to_string(&shift)?),
                ))
                .execute(conn)?;
            Ok(shift)
        })
    }
    pub fn move_cash(&mut self, movement: &CashMovement) -> Result<()> {
        let session = self.session.clone();
        let register = self.register;
        self.conn.immediate_transaction(|conn| {
            if !matches!(movement.kind, CashKind::CashIn | CashKind::CashOut)
                || movement.payment_method != pos_domain::PaymentMethod::Cash
                || movement.amount == Money::ZERO
                || movement.reason.trim().is_empty()
                || movement.reason.chars().count() > 240
            {
                return Err(Error::InvalidCashMovement);
            }
            if let Some(json) = cash_movements::table
                .find(movement.id.to_string())
                .select(cash_movements::movement_json)
                .first::<String>(conn)
                .optional()?
            {
                let previous: CashMovement = serde_json::from_str(&json)?;
                // A retry may have a later clock time, but never a different financial payload.
                return if previous.shift_id == movement.shift_id
                    && previous.kind == movement.kind
                    && previous.payment_method == movement.payment_method
                    && previous.amount == movement.amount
                    && previous.reason == movement.reason
                {
                    Ok(())
                } else {
                    Err(Error::ConflictingRequest)
                };
            }
            let shift = active(conn, register, Some(movement.shift_id), movement.created_at)?;
            let staff = crate::staff::authorize(
                conn,
                &session,
                Some(pos_domain::Permission::CashMovement),
            )?;
            record(
                conn,
                shift,
                CashMovement {
                    staff,
                    ..movement.clone()
                },
            )
        })
    }
    /// Revision prevents confirming totals that changed on another connection.
    /// Records the count without a reason; the desktop uses `close_shift_with`.
    pub fn close_shift(
        &mut self,
        id: Uuid,
        revision: u64,
        counted: Money,
        now: i64,
    ) -> Result<Shift> {
        let count = ShiftCount {
            counted,
            reason: String::new(),
            denominations: Vec::new(),
        };
        self.close(id, revision, &count, false, now)
    }
    /// Closing with a reason (required when the count is off) and an optional
    /// denomination breakdown that must add up to the counted total.
    pub fn close_shift_with(
        &mut self,
        id: Uuid,
        revision: u64,
        count: &ShiftCount,
        now: i64,
    ) -> Result<Shift> {
        self.close(id, revision, count, true, now)
    }
    fn close(
        &mut self,
        id: Uuid,
        revision: u64,
        count: &ShiftCount,
        require_reason: bool,
        now: i64,
    ) -> Result<Shift> {
        let counted = count.counted;
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            let json = shifts::table
                .find(id.to_string())
                .select(shifts::shift_json)
                .first::<String>(conn)
                .optional()?
                .ok_or(Error::ShiftClosed)?;
            let mut shift: Shift = serde_json::from_str(&json)?;
            if let Some(closing) = &shift.closing {
                return if closing.counted == counted {
                    Ok(shift)
                } else {
                    Err(Error::ConflictingRequest)
                };
            }
            if shift.revision != revision {
                return Err(Error::ShiftChanged);
            }
            if now < shift.last_activity_at {
                return Err(Error::InvalidShiftTime);
            }
            let staff =
                crate::staff::authorize(conn, &session, Some(pos_domain::Permission::CloseShift))?;
            let difference = shift.difference(counted)?;
            count.validate(if require_reason {
                difference
            } else {
                SignedMoney::default()
            })?;
            shift.closing = Some(ShiftClosing {
                closed_at: now,
                expected: shift.expected()?,
                counted,
                difference,
                reason: count.reason.trim().into(),
                denominations: count.denominations.clone(),
                closed_by: staff.by,
            });
            save(conn, &shift)?;
            Ok(shift)
        })
    }
}
