//! Staff, roles and what each role may do. A shop without staff works as
//! before (no sign-in, no attribution). With staff, every money-moving record
//! snapshots who did it and, for restricted actions, who approved it.
//!
//! Roles are shop-defined: each permission is Allow, Approval (another
//! member who may approve it signs with a PIN) or Deny. The owner role is
//! fixed and allows everything, so a shop can never lock itself out.
use crate::DomainError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Built-in role IDs. Custom roles use UUID strings.
pub const OWNER: &str = "owner";
pub const MANAGER: &str = "manager";
pub const CASHIER: &str = "cashier";

/// Actions a role may be allowed, need approval for, or be denied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    /// Refund some items of a sale.
    Refund,
    /// Void a whole sale.
    Void,
    /// Discounts above the role's own limit.
    Discount,
    /// Cash in / cash out of the drawer.
    CashMovement,
    /// Count the drawer and close the shift.
    CloseShift,
    /// Add, edit, archive products.
    Catalog,
    /// Receive, remove or count stock.
    Stock,
    /// See reports and export them.
    Reports,
    /// Shop, printer, pricing and VAT settings; backups.
    Settings,
    /// Staff, roles and PINs.
    ManageStaff,
}
impl Permission {
    pub const ALL: [Permission; 10] = [
        Self::Refund,
        Self::Void,
        Self::Discount,
        Self::CashMovement,
        Self::CloseShift,
        Self::Catalog,
        Self::Stock,
        Self::Reports,
        Self::Settings,
        Self::ManageStaff,
    ];
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    Allow,
    /// Needs a PIN from a member whose role allows it and may approve.
    Approval,
    Deny,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleDef {
    pub id: String,
    pub name: String,
    pub access: BTreeMap<Permission, Access>,
    /// May approve, with a PIN, what this role itself is allowed.
    pub can_approve: bool,
    /// Discounts up to this share of the bill (basis points) need nothing;
    /// above it the Discount permission applies.
    pub discount_limit_bp: u32,
}
impl RoleDef {
    pub fn owner() -> Self {
        Self {
            id: OWNER.into(),
            name: "เจ้าของร้าน / Owner".into(),
            access: Permission::ALL
                .iter()
                .map(|p| (*p, Access::Allow))
                .collect(),
            can_approve: true,
            discount_limit_bp: 10_000,
        }
    }
    pub fn manager() -> Self {
        let mut access: BTreeMap<_, _> = Permission::ALL
            .iter()
            .map(|p| (*p, Access::Allow))
            .collect();
        access.insert(Permission::ManageStaff, Access::Deny);
        Self {
            id: MANAGER.into(),
            name: "ผู้จัดการ / Manager".into(),
            access,
            can_approve: true,
            discount_limit_bp: 10_000,
        }
    }
    pub fn cashier(discount_limit_bp: u32) -> Self {
        use Access::*;
        use Permission::*;
        let access = [
            (Refund, Approval),
            (Void, Approval),
            (Discount, Approval),
            (CashMovement, Approval),
            (CloseShift, Allow),
            (Catalog, Approval),
            (Stock, Approval),
            (Reports, Allow),
            (Settings, Deny),
            (ManageStaff, Deny),
        ]
        .into_iter()
        .collect();
        Self {
            id: CASHIER.into(),
            name: "แคชเชียร์ / Cashier".into(),
            access,
            can_approve: false,
            discount_limit_bp,
        }
    }
    pub fn is_owner(&self) -> bool {
        self.id == OWNER
    }
    /// Missing entries (permissions added by later versions) need approval,
    /// except for the owner.
    pub fn access(&self, p: Permission) -> Access {
        if self.is_owner() {
            return Access::Allow;
        }
        self.access.get(&p).copied().unwrap_or(Access::Approval)
    }
    pub fn allows(&self, p: Permission) -> bool {
        self.access(p) == Access::Allow
    }
    /// May sign an approval for `p`.
    pub fn approves(&self, p: Permission) -> bool {
        (self.is_owner() || self.can_approve) && self.allows(p)
    }
    pub fn validate(&self) -> Result<(), DomainError> {
        let n = self.name.trim().chars().count();
        if n == 0 || n > 40 || self.id.trim().is_empty() || self.discount_limit_bp > 10_000 {
            return Err(DomainError::InvalidRole);
        }
        // Only the owner manages staff, so roles cannot grant it.
        if !self.is_owner() && self.access(Permission::ManageStaff) != Access::Deny {
            return Err(DomainError::InvalidRole);
        }
        Ok(())
    }
}
/// Immutable snapshot of a staff member on a record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaffRef {
    pub id: Uuid,
    pub name: String,
    /// Role ID at the time ("owner", "manager", "cashier" or a custom ID).
    pub role: String,
    /// Role name at the time; empty on snapshots from built-in-only versions.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub role_name: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Staff {
    pub id: Uuid,
    pub name: String,
    pub role: String,
    pub active: bool,
}
impl Staff {
    pub fn reference(&self, role_name: &str) -> StaffRef {
        StaffRef {
            id: self.id,
            name: self.name.clone(),
            role: self.role.clone(),
            role_name: role_name.to_owned(),
        }
    }
}
#[derive(Debug, Clone, Default)]
pub struct StaffInput {
    pub id: Option<Uuid>,
    pub name: String,
    pub role: Option<String>,
    /// New PIN; None keeps the current one (required for new staff).
    pub pin: Option<String>,
    pub active: bool,
}
pub fn valid_pin(pin: &str) -> bool {
    (4..=8).contains(&pin.len()) && pin.bytes().all(|b| b.is_ascii_digit())
}
impl StaffInput {
    pub fn validate(&self) -> Result<(), DomainError> {
        let n = self.name.trim().chars().count();
        if n == 0 || n > 60 || self.role.as_deref().is_none_or(str::is_empty) {
            return Err(DomainError::InvalidStaff);
        }
        if self.id.is_none() && self.pin.is_none() {
            return Err(DomainError::InvalidPin);
        }
        if self.pin.as_deref().is_some_and(|p| !valid_pin(p)) {
            return Err(DomainError::InvalidPin);
        }
        Ok(())
    }
}
/// Who is acting, and who approved when the actor alone may not.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Attribution {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<StaffRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_by: Option<StaffRef>,
}
impl Attribution {
    pub fn is_empty(&self) -> bool {
        self.by.is_none() && self.approved_by.is_none()
    }
}
/// Discount share of the bill before discounts, in basis points (rounded up),
/// compared with the role's limit.
pub fn discount_share_bp(discounts: crate::Money, subtotal: crate::Money) -> u32 {
    if subtotal.satang() == 0 {
        return if discounts.satang() > 0 { 10_000 } else { 0 };
    }
    let bp = (i128::from(discounts.satang()) * 10_000 + i128::from(subtotal.satang()) - 1)
        / i128::from(subtotal.satang());
    u32::try_from(bp).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roles_and_pins() {
        let owner = RoleDef::owner();
        let manager = RoleDef::manager();
        let cashier = RoleDef::cashier(1_000);
        assert!(owner.approves(Permission::ManageStaff));
        assert!(manager.approves(Permission::Refund));
        assert!(!manager.allows(Permission::ManageStaff));
        assert_eq!(cashier.access(Permission::Discount), Access::Approval);
        assert!(!cashier.approves(Permission::Reports));
        for r in [&owner, &manager, &cashier] {
            assert_eq!(r.validate(), Ok(()));
        }
        // A custom role cannot grant staff management.
        let mut lead = RoleDef::cashier(0);
        lead.access.insert(Permission::ManageStaff, Access::Allow);
        assert_eq!(lead.validate(), Err(DomainError::InvalidRole));
        // Permissions unknown to a stored role need approval.
        lead.access.clear();
        assert_eq!(lead.access(Permission::Refund), Access::Approval);
        assert!(valid_pin("1234") && valid_pin("12345678"));
        assert!(!valid_pin("123") && !valid_pin("12a4") && !valid_pin("123456789"));
        let input = StaffInput {
            name: "นิด".into(),
            role: Some(CASHIER.into()),
            pin: None,
            ..Default::default()
        };
        assert_eq!(input.validate(), Err(DomainError::InvalidPin));
    }
    #[test]
    fn old_snapshots_still_read() -> Result<(), serde_json::Error> {
        let old: StaffRef = serde_json::from_str(
            r#"{"id":"00000000-0000-0000-0000-000000000001","name":"นิด","role":"cashier"}"#,
        )?;
        assert_eq!((old.role.as_str(), old.role_name.as_str()), (CASHIER, ""));
        Ok(())
    }
    #[test]
    fn discount_share_rounds_up() {
        let m = |s: &str| s.parse().unwrap_or_default();
        assert_eq!(discount_share_bp(m("10"), m("100")), 1_000);
        assert_eq!(discount_share_bp(m("10.01"), m("100")), 1_001);
        assert_eq!(discount_share_bp(m("1"), m("3")), 3_334);
        assert_eq!(discount_share_bp(m("0"), m("0")), 0);
    }
}
