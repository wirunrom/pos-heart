//! Tables, zones and kitchen tickets for sit-down service.
//!
//! A table's open order is an ordinary held bill tagged with the table. The
//! bill remembers what the kitchen already has (`Cart::sent`), so each send
//! fires only the difference: more units become a new ticket, fewer units a
//! cancellation ticket. Receipts and payment work exactly as for any bill.
use crate::{Cart, DomainError, LineOption};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiningTable {
    pub id: Uuid,
    /// Area such as "ในร้าน" or "ระเบียง"; tables group by zone.
    pub zone: String,
    pub name: String,
    pub seats: u32,
    pub active: bool,
}
impl DiningTable {
    pub fn validate(&self) -> Result<(), DomainError> {
        let name = self.name.trim().chars().count();
        if name == 0 || name > 20 || self.zone.trim().chars().count() > 40 || self.seats > 99 {
            return Err(DomainError::InvalidTable);
        }
        Ok(())
    }
}
/// What the kitchen has for one (product, options, note) combination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SentItem {
    pub product_id: Uuid,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<LineOption>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    pub quantity: u32,
}
impl SentItem {
    fn same(&self, product_id: Uuid, options: &[LineOption], note: &str) -> bool {
        self.product_id == product_id && self.options == options && self.note == note
    }
}
/// Units to fire (positive) or cancel (negative) per combination, and the
/// kitchen's new state. `kitchen(product)` says which products the kitchen
/// prepares; others (bottled drinks, retail) never go to the kitchen.
pub fn kitchen_changes(
    cart: &Cart,
    kitchen: impl Fn(Uuid) -> bool,
) -> (Vec<(SentItem, i64)>, Vec<SentItem>) {
    let mut now: Vec<SentItem> = Vec::new();
    for line in cart.lines.iter().filter(|l| kitchen(l.product_id)) {
        match now
            .iter_mut()
            .find(|s| s.same(line.product_id, &line.options, &line.note))
        {
            Some(s) => s.quantity = s.quantity.saturating_add(line.quantity),
            None => now.push(SentItem {
                product_id: line.product_id,
                options: line.options.clone(),
                note: line.note.clone(),
                quantity: line.quantity,
            }),
        }
    }
    let mut changes = Vec::new();
    for item in &now {
        let before = cart
            .sent
            .iter()
            .find(|s| s.same(item.product_id, &item.options, &item.note))
            .map_or(0, |s| s.quantity);
        let delta = i64::from(item.quantity) - i64::from(before);
        if delta != 0 {
            changes.push((item.clone(), delta));
        }
    }
    for old in &cart.sent {
        let gone = !now
            .iter()
            .any(|s| s.same(old.product_id, &old.options, &old.note));
        if gone && old.quantity > 0 {
            changes.push((old.clone(), -i64::from(old.quantity)));
        }
    }
    (changes, now)
}
/// One line of a kitchen ticket. Negative quantities are cancellations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TicketLine {
    pub product_id: Uuid,
    pub name: String,
    pub quantity: i64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    #[serde(default)]
    pub done: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitchenTicket {
    pub id: Uuid,
    pub number: u64,
    pub cart_id: Uuid,
    /// Table name at the time, or the bill name for counter orders.
    pub table: String,
    #[serde(default)]
    pub guests: u32,
    pub created_at: i64,
    pub lines: Vec<TicketLine>,
    /// Set when the kitchen marks the whole ticket done.
    #[serde(default)]
    pub bumped_at: Option<i64>,
    #[serde(default, skip_serializing_if = "crate::Attribution::is_empty")]
    pub staff: crate::Attribution,
}
impl KitchenTicket {
    pub fn is_cancellation(&self) -> bool {
        self.lines.iter().all(|l| l.quantity < 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CartLine;
    fn line(p: u128, q: u32, note: &str) -> CartLine {
        CartLine {
            product_id: Uuid::from_u128(p),
            quantity: q,
            note: note.into(),
            ..Default::default()
        }
    }
    #[test]
    fn sends_fire_only_the_difference() {
        let mut cart = Cart::new();
        cart.lines = vec![
            line(1, 2, ""),
            line(1, 1, ""),
            line(2, 1, "ไม่เผ็ด"),
            line(9, 3, ""),
        ];
        // Product 9 (bottled water) never goes to the kitchen.
        let kitchen = |p: Uuid| p != Uuid::from_u128(9);
        let (changes, sent) = kitchen_changes(&cart, kitchen);
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].1, 3); // two lines of the same dish merge
        cart.sent = sent;
        assert!(kitchen_changes(&cart, kitchen).0.is_empty());
        // One more dish, and the spicy-free one removed: fire 1, cancel 1.
        cart.lines = vec![line(1, 4, "")];
        let (changes, _) = kitchen_changes(&cart, kitchen);
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].1, 1);
        assert_eq!(changes[1].1, -1);
        assert_eq!(changes[1].0.note, "ไม่เผ็ด");
    }
    #[test]
    fn tables_are_validated() {
        let mut t = DiningTable {
            id: Uuid::nil(),
            zone: "ในร้าน".into(),
            name: "T1".into(),
            seats: 4,
            active: true,
        };
        assert!(t.validate().is_ok());
        t.name = " ".into();
        assert_eq!(t.validate(), Err(DomainError::InvalidTable));
    }
}
