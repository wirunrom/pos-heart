//! Staff roster, PIN sign-in and authorization of restricted actions.
//! PINs are stored as salted PBKDF2-SHA256 hashes. A 4–8 digit PIN only
//! identifies who is at the counter; it does not protect the database file
//! from someone who can copy it.
use crate::{
    Error, Pos, Result,
    schema::{app_settings, roles, staff},
};
use diesel::prelude::*;
use pos_domain::{
    Access, Attribution, CASHIER, MANAGER, OWNER, Permission, RoleDef, Staff, StaffInput, StaffRef,
};
use sha2::Sha256;
use std::time::{Duration, Instant};
use uuid::Uuid;

const ITERATIONS: u32 = 60_000;
const MAX_FAILURES: u32 = 5;
const LOCKOUT: Duration = Duration::from_secs(30);
/// Default cashier discount limit: 10% of the bill before discounts.
pub const DEFAULT_DISCOUNT_LIMIT_BP: u32 = 1_000;
/// Shop-wide cashier limit from schema 9, carried into the cashier role.
const LEGACY_LIMIT_KEY: &str = "cashier_discount_limit_bp";

#[derive(Debug, Clone, Default)]
pub(crate) struct Session {
    pub actor: Option<Uuid>,
    /// One-shot approval for the next command; the caller clears it.
    pub approver: Option<Uuid>,
    failures: u32,
    locked_until: Option<Instant>,
}

fn hash(pin: &str, salt: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha256>(pin.as_bytes(), salt, ITERATIONS, &mut out);
    out
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}
fn encode_pin(pin: &str) -> String {
    let salt = *Uuid::new_v4().as_bytes();
    format!(
        "pbkdf2-sha256${ITERATIONS}${}${}",
        hex(&salt),
        hex(&hash(pin, &salt))
    )
}
fn pin_matches(stored: &str, pin: &str) -> bool {
    let mut parts = stored.split('$');
    let (Some("pbkdf2-sha256"), Some(iter), Some(salt), Some(expected), None) = (
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
    ) else {
        return false;
    };
    let (Ok(iterations), Some(salt), Some(expected)) =
        (iter.parse::<u32>(), unhex(salt), unhex(expected))
    else {
        return false;
    };
    let mut out = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha256>(pin.as_bytes(), &salt, iterations, &mut out);
    // Constant-time comparison.
    expected.len() == out.len() && expected.iter().zip(out).fold(0u8, |a, (x, y)| a | (x ^ y)) == 0
}

type Row = (String, String, String, String, i32);
fn from_row((id, name, role, _hash, active): &Row) -> Result<Staff> {
    Ok(Staff {
        id: Uuid::parse_str(id).map_err(|_| Error::CorruptData)?,
        name: name.clone(),
        role: role.clone(),
        active: *active == 1,
    })
}
fn load(conn: &mut SqliteConnection, id: Uuid) -> Result<Option<(Staff, String)>> {
    staff::table
        .find(id.to_string())
        .select((
            staff::id,
            staff::name,
            staff::role,
            staff::pin_hash,
            staff::active,
        ))
        .first::<Row>(conn)
        .optional()?
        .map(|row| Ok((from_row(&row)?, row.3)))
        .transpose()
}
/// Creates the built-in manager and cashier roles once; the cashier keeps
/// the shop-wide discount limit an older version stored.
pub(crate) fn ensure_roles(conn: &mut SqliteConnection) -> Result<()> {
    let legacy = app_settings::table
        .find(LEGACY_LIMIT_KEY)
        .select(app_settings::value)
        .first::<String>(conn)
        .optional()?
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_DISCOUNT_LIMIT_BP);
    for role in [RoleDef::manager(), RoleDef::cashier(legacy)] {
        let exists = diesel::select(diesel::dsl::exists(roles::table.find(&role.id)))
            .get_result::<bool>(conn)?;
        if !exists {
            write_role(conn, &role)?;
        }
    }
    Ok(())
}
fn write_role(conn: &mut SqliteConnection, role: &RoleDef) -> Result<()> {
    diesel::replace_into(roles::table)
        .values((
            roles::id.eq(&role.id),
            roles::name.eq(role.name.trim()),
            roles::role_json.eq(serde_json::to_string(role)?),
        ))
        .execute(conn)?;
    Ok(())
}
pub(crate) fn role(conn: &mut SqliteConnection, id: &str) -> Result<Option<RoleDef>> {
    if id == OWNER {
        return Ok(Some(RoleDef::owner()));
    }
    roles::table
        .find(id)
        .select(roles::role_json)
        .first::<String>(conn)
        .optional()?
        .map(|json| Ok(serde_json::from_str(&json)?))
        .transpose()
}
pub(crate) fn enabled(conn: &mut SqliteConnection) -> Result<bool> {
    Ok(diesel::select(diesel::dsl::exists(
        staff::table.filter(staff::active.eq(1)),
    ))
    .get_result::<bool>(conn)?)
}
/// An active member with their current role (roles take effect at once).
fn active_member(
    conn: &mut SqliteConnection,
    id: Option<Uuid>,
) -> Result<Option<(Staff, RoleDef)>> {
    let Some(member) = id
        .map(|id| load(conn, id))
        .transpose()?
        .flatten()
        .map(|(s, _)| s)
        .filter(|s| s.active)
    else {
        return Ok(None);
    };
    let role = role(conn, &member.role)?.ok_or(Error::RoleNotFound)?;
    Ok(Some((member, role)))
}
/// Who is acting for a write, checked inside its transaction. Without staff
/// the shop works unattributed. With staff, someone must be signed in; a
/// restricted `permission` is allowed, needs an approver's PIN, or is denied
/// according to the actor's role.
pub(crate) fn authorize(
    conn: &mut SqliteConnection,
    session: &Session,
    permission: Option<Permission>,
) -> Result<Attribution> {
    if !enabled(conn)? {
        return Ok(Attribution::default());
    }
    let (actor, actor_role) = active_member(conn, session.actor)?.ok_or(Error::SignInRequired)?;
    let by = Some(actor.reference(&actor_role.name));
    let Some(permission) = permission else {
        return Ok(Attribution {
            by,
            approved_by: None,
        });
    };
    match actor_role.access(permission) {
        Access::Allow => Ok(Attribution {
            by,
            approved_by: None,
        }),
        Access::Deny => Err(Error::Forbidden(permission)),
        Access::Approval => match active_member(conn, session.approver)? {
            Some((a, r)) if r.approves(permission) && a.id != actor.id => Ok(Attribution {
                by,
                approved_by: Some(a.reference(&r.name)),
            }),
            _ => Err(Error::ApprovalRequired(permission)),
        },
    }
}
/// The signed-in member's discount limit (basis points); everything
/// without staff.
pub(crate) fn discount_limit(conn: &mut SqliteConnection, session: &Session) -> Result<u32> {
    if !enabled(conn)? {
        return Ok(10_000);
    }
    Ok(active_member(conn, session.actor)?.map_or(0, |(_, r)| {
        if r.allows(Permission::Discount) {
            10_000
        } else {
            r.discount_limit_bp
        }
    }))
}

impl Pos {
    /// Everyone on the roster, active first, then owners, then by name.
    pub fn staff(&mut self) -> Result<Vec<Staff>> {
        let rows = staff::table
            .select((
                staff::id,
                staff::name,
                staff::role,
                staff::pin_hash,
                staff::active,
            ))
            .load::<Row>(&mut self.conn)?;
        let mut out = rows.iter().map(from_row).collect::<Result<Vec<_>>>()?;
        out.sort_by(|a, b| {
            b.active
                .cmp(&a.active)
                .then((b.role == OWNER).cmp(&(a.role == OWNER)))
                .then(a.name.cmp(&b.name))
        });
        Ok(out)
    }
    /// Owner first, then manager, cashier and custom roles by name.
    pub fn roles(&mut self) -> Result<Vec<RoleDef>> {
        let mut stored = roles::table
            .select(roles::role_json)
            .load::<String>(&mut self.conn)?
            .iter()
            .map(|json| Ok(serde_json::from_str::<RoleDef>(json)?))
            .collect::<Result<Vec<_>>>()?;
        let rank = |r: &RoleDef| match r.id.as_str() {
            MANAGER => 0,
            CASHIER => 1,
            _ => 2,
        };
        stored.sort_by(|a, b| rank(a).cmp(&rank(b)).then(a.name.cmp(&b.name)));
        let mut out = vec![RoleDef::owner()];
        out.extend(stored);
        Ok(out)
    }
    /// Adds (empty ID) or edits a role. Owner-only; the owner role is fixed.
    pub fn save_role(&mut self, role: &RoleDef) -> Result<RoleDef> {
        let mut role = role.clone();
        if role.id.trim().is_empty() {
            role.id = Uuid::new_v4().to_string();
        }
        role.name = role.name.trim().to_owned();
        role.validate()?;
        if role.is_owner() {
            return Err(pos_domain::DomainError::InvalidRole.into());
        }
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            let attribution = authorize(conn, &session, Some(Permission::ManageStaff))?;
            let previous = roles::table.find(&role.id).select(roles::role_json).first::<String>(conn).optional()?;
            write_role(conn, &role)?;
            crate::audit::record(conn, crate::ActivityKind::Role, &role.id, crate::audit::now(), &attribution,
                &serde_json::json!({"action": "save", "before": previous.map(|json| serde_json::from_str::<RoleDef>(&json)).transpose()?, "after": role}))?;
            Ok(role)
        })
    }
    /// Removes a custom role that nobody (active or not) holds.
    pub fn delete_role(&mut self, id: &str) -> Result<()> {
        if [OWNER, MANAGER, CASHIER].contains(&id) {
            return Err(pos_domain::DomainError::InvalidRole.into());
        }
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            let attribution = authorize(conn, &session, Some(Permission::ManageStaff))?;
            let previous = roles::table.find(id).select(roles::role_json).first::<String>(conn).optional()?;
            let used = diesel::select(diesel::dsl::exists(staff::table.filter(staff::role.eq(id))))
                .get_result::<bool>(conn)?;
            if used {
                return Err(Error::RoleInUse);
            }
            diesel::delete(roles::table.find(id)).execute(conn)?;
            if let Some(previous) = previous {
                crate::audit::record(conn, crate::ActivityKind::Role, id, crate::audit::now(), &attribution,
                    &serde_json::json!({"action": "delete", "before": serde_json::from_str::<RoleDef>(&previous)?}))?;
            }
            Ok(())
        })
    }
    /// True when sign-in is required (any active staff).
    pub fn staff_enabled(&mut self) -> Result<bool> {
        enabled(&mut self.conn)
    }
    /// Adds or edits a staff member. The first one must be an owner and is
    /// signed in; afterwards only an owner (or an owner's approval) may edit.
    /// Active staff always keep at least one active owner.
    pub fn save_staff(&mut self, input: &StaffInput, now: i64) -> Result<Staff> {
        input.validate()?;
        let session = self.session.clone();
        let saved = self.conn.immediate_transaction(|conn| {
            let first = !enabled(conn)?;
            let attribution = if first { Attribution::default() } else {
                authorize(conn, &session, Some(Permission::ManageStaff))?
            };
            let role_id = input
                .role
                .clone()
                .ok_or(pos_domain::DomainError::InvalidStaff)?;
            if role(conn, &role_id)?.is_none() {
                return Err(Error::RoleNotFound);
            }
            if first && (role_id != OWNER || !input.active) {
                return Err(Error::LastOwner);
            }
            let id = input.id.unwrap_or_else(Uuid::new_v4);
            let existing = load(conn, id)?;
            if input.id.is_some() && existing.is_none() {
                return Err(Error::StaffNotFound);
            }
            let pin_hash = match (&input.pin, &existing) {
                (Some(pin), _) => encode_pin(pin),
                (None, Some((_, hash))) => hash.clone(),
                (None, None) => return Err(pos_domain::DomainError::InvalidPin.into()),
            };
            let active = i32::from(input.active);
            let name = input.name.trim().to_owned();
            if existing.is_some() {
                diesel::update(staff::table.find(id.to_string()))
                    .set((
                        staff::name.eq(&name),
                        staff::role.eq(&role_id),
                        staff::pin_hash.eq(&pin_hash),
                        staff::active.eq(active),
                    ))
                    .execute(conn)?;
            } else {
                diesel::insert_into(staff::table)
                    .values((
                        staff::id.eq(id.to_string()),
                        staff::name.eq(&name),
                        staff::role.eq(&role_id),
                        staff::pin_hash.eq(&pin_hash),
                        staff::active.eq(active),
                        staff::created_at.eq(now),
                    ))
                    .execute(conn)?;
            }
            let owners: i64 = staff::table
                .filter(staff::active.eq(1))
                .filter(staff::role.eq(OWNER))
                .count()
                .get_result(conn)?;
            if enabled(conn)? && owners == 0 {
                return Err(Error::LastOwner);
            }
            crate::audit::record(conn, crate::ActivityKind::Staff, &id.to_string(), now, &attribution,
                &serde_json::json!({"action": "save", "before": existing.as_ref().map(|(s, _)| serde_json::json!({"id": s.id, "name": s.name, "role": s.role, "active": s.active})),
                    "after": {"id": id, "name": name, "role": role_id, "active": input.active}, "pin_changed": input.pin.is_some()}))?;
            Ok((
                Staff {
                    id,
                    name,
                    role: role_id,
                    active: input.active,
                },
                first,
            ))
        })?;
        let (member, first) = saved;
        if first {
            self.session.actor = Some(member.id);
        }
        Ok(member)
    }
    /// Checks the PIN and signs the member in. Five wrong PINs in a row lock
    /// sign-in and approvals for 30 seconds.
    pub fn sign_in(&mut self, id: Uuid, pin: &str) -> Result<StaffRef> {
        let (member, role) = self.check_pin(id, pin)?;
        self.session.actor = Some(member.id);
        self.session.approver = None;
        Ok(member.reference(&role.name))
    }
    pub fn sign_out(&mut self) {
        self.session.actor = None;
        self.session.approver = None;
    }
    /// The signed-in member, if still active.
    pub fn signed_in(&mut self) -> Result<Option<StaffRef>> {
        let id = self.session.actor;
        Ok(active_member(&mut self.conn, id)?.map(|(s, r)| s.reference(&r.name)))
    }
    /// Verifies an approver's PIN for the next restricted command. Members
    /// cannot approve their own request.
    pub fn approve(&mut self, id: Uuid, pin: &str, permission: Permission) -> Result<StaffRef> {
        if self.session.actor == Some(id) {
            return Err(Error::ApprovalRequired(permission));
        }
        let (member, role) = self.check_pin(id, pin)?;
        if !role.approves(permission) {
            return Err(Error::ApprovalRequired(permission));
        }
        self.session.approver = Some(member.id);
        Ok(member.reference(&role.name))
    }
    pub fn clear_approval(&mut self) {
        self.session.approver = None;
    }
    fn check_pin(&mut self, id: Uuid, pin: &str) -> Result<(Staff, RoleDef)> {
        if let Some(until) = self.session.locked_until {
            let now = Instant::now();
            if now < until {
                return Err(Error::PinLocked((until - now).as_secs().max(1)));
            }
            self.session.locked_until = None;
            self.session.failures = 0;
        }
        let found = load(&mut self.conn, id)?.filter(|(s, _)| s.active);
        match found {
            Some((member, hash)) if pin_matches(&hash, pin) => {
                self.session.failures = 0;
                let role = role(&mut self.conn, &member.role)?.ok_or(Error::RoleNotFound)?;
                Ok((member, role))
            }
            Some(_) => {
                self.session.failures += 1;
                if self.session.failures >= MAX_FAILURES {
                    self.session.locked_until = Some(Instant::now() + LOCKOUT);
                }
                Err(Error::WrongPin)
            }
            None => Err(Error::StaffNotFound),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pins_hash_with_salt_and_verify() {
        let a = encode_pin("1234");
        let b = encode_pin("1234");
        assert_ne!(a, b);
        assert!(pin_matches(&a, "1234"));
        assert!(!pin_matches(&a, "1235"));
        assert!(!pin_matches("garbage", "1234"));
        assert!(!pin_matches("pbkdf2-sha256$1$zz$00", "1234"));
    }
}
