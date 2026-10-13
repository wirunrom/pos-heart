use crate::{DomainError, Money};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: Uuid,
    pub sku: String,
    pub name: String,
    pub category: String,
    pub price: Money,
    /// Standard cost per sold unit, including any typical option costs. None is unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<Money>,
    /// None for made-to-order items. Some(n) tracks individual retail units.
    pub stock: Option<u32>,
    /// False only after archiving; archived products leave the catalog entirely.
    pub active: bool,
    /// Scanner code, distinct from the shop's own product code (SKU).
    #[serde(default)]
    pub barcode: Option<String>,
    /// "Not for sale" keeps the product listed but refuses checkout.
    #[serde(default = "on_sale_default")]
    pub on_sale: bool,
    /// Option groups offered when adding this product (sweetness, toppings…).
    #[serde(default)]
    pub options: Vec<crate::OptionGroup>,
    /// Service charge applies (off for retail goods in a restaurant).
    #[serde(default = "on_sale_default")]
    pub service_charge: bool,
    /// Prepared by the kitchen (sent on kitchen tickets).
    #[serde(default = "on_sale_default")]
    pub kitchen: bool,
    /// Ingredients one unit uses; deducted at checkout.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recipe: Vec<crate::RecipeItem>,
    /// Recipe changes by option choice ("+ egg", "no egg").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub option_recipe: Vec<crate::OptionRecipe>,
}
fn on_sale_default() -> bool {
    true
}
/// Tracked stock at or below this many units is shown as low stock.
pub const LOW_STOCK: u32 = 5;
impl Product {
    pub fn is_low_stock(&self) -> bool {
        self.stock.is_some_and(|n| n > 0 && n <= LOW_STOCK)
    }
    pub fn is_out_of_stock(&self) -> bool {
        self.stock == Some(0)
    }
    /// Unit price for a line with these options, checked against this product.
    pub fn unit_price(&self, chosen: &[crate::LineOption]) -> Result<Money, DomainError> {
        crate::price_with_options(self.price, &self.options, chosen).map(|(unit, _)| unit)
    }
}
#[derive(Debug, Clone)]
pub struct ProductInput {
    pub id: Option<Uuid>,
    pub sku: String,
    pub name: String,
    pub category: String,
    pub price: Money,
    pub cost: Option<Money>,
    pub stock: Option<u32>,
    pub barcode: Option<String>,
    pub on_sale: bool,
    pub options: Vec<crate::OptionGroup>,
    pub service_charge: bool,
    pub kitchen: bool,
    pub recipe: Vec<crate::RecipeItem>,
    pub option_recipe: Vec<crate::OptionRecipe>,
    /// When the product already tracks stock and keeps tracking it, leave the
    /// stored count alone (changes go through stock movements). The desktop
    /// sets this so saving an edit never overwrites sales made meanwhile.
    pub keep_tracked_stock: bool,
}
impl Default for ProductInput {
    fn default() -> Self {
        Self {
            id: None,
            sku: String::new(),
            name: String::new(),
            category: String::new(),
            price: Money::ZERO,
            cost: None,
            stock: None,
            barcode: None,
            on_sale: true,
            options: Vec::new(),
            service_charge: true,
            kitchen: true,
            recipe: Vec::new(),
            option_recipe: Vec::new(),
            keep_tracked_stock: false,
        }
    }
}
impl ProductInput {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.sku.trim().is_empty() || self.name.trim().is_empty() {
            return Err(DomainError::MissingProductFields);
        }
        if self.sku.chars().count() > 64
            || self.name.chars().count() > 160
            || self.category.chars().count() > 80
            || self
                .barcode
                .as_ref()
                .is_some_and(|b| b.chars().count() > 64)
        {
            return Err(DomainError::TextTooLong);
        }
        crate::validate_recipe(&self.recipe)?;
        crate::validate_option_recipe(&self.option_recipe, &self.options)?;
        crate::validate_groups(&self.options)
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartLine {
    pub product_id: Uuid,
    pub quantity: u32,
    pub note: String,
    /// Chosen options; lines with different options stay separate.
    #[serde(default)]
    pub options: Vec<crate::LineOption>,
    /// Discount on this line (all units).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discount: Option<crate::Discount>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cart {
    /// Stable across retries. The same ID can be paid only once.
    pub id: Uuid,
    pub label: String,
    pub lines: Vec<CartLine>,
    /// Discount on the whole bill, after line discounts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discount: Option<crate::Discount>,
    /// Why discounts were given (member, promotion…). Optional.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub discount_reason: String,
    /// Service charge for this bill (off e.g. for takeaway).
    #[serde(default = "on_sale_default")]
    pub service_charge: bool,
    /// Dining table holding this bill, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<Uuid>,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub guests: u32,
    /// What the kitchen already has for this bill.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sent: Vec<crate::SentItem>,
    /// Coupon codes entered on this bill (for coupon promotions).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coupons: Vec<String>,
}
impl Default for Cart {
    fn default() -> Self {
        Self::new()
    }
}
impl Cart {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            label: String::new(),
            lines: Vec::new(),
            discount: None,
            discount_reason: String::new(),
            service_charge: true,
            table: None,
            guests: 0,
            sent: Vec::new(),
            coupons: Vec::new(),
        }
    }
    /// True when any line or the bill has a discount.
    pub fn has_discount(&self) -> bool {
        self.discount.is_some() || self.lines.iter().any(|l| l.discount.is_some())
    }
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.lines.is_empty() {
            return Err(DomainError::EmptyCart);
        }
        if self.label.chars().count() > 160
            || self.lines.len() > 500
            || self.discount_reason.chars().count() > 240
            || self.coupons.len() > 5
            || self.coupons.iter().any(|c| c.chars().count() > 32)
        {
            return Err(DomainError::TextTooLong);
        }
        if let Some(d) = &self.discount {
            d.validate()?;
        }
        for line in &self.lines {
            if line.quantity == 0 || line.quantity > 999_999 {
                return Err(DomainError::InvalidQuantity);
            }
            if line.note.chars().count() > 240 {
                return Err(DomainError::TextTooLong);
            }
            if line.options.len() > crate::options::MAX_GROUPS * crate::options::MAX_CHOICES {
                return Err(DomainError::InvalidOptions);
            }
            if let Some(d) = &line.discount {
                d.validate()?;
            }
        }
        Ok(())
    }
}
/// Moves `take` (line index, units) of `cart` to a new bill. The kitchen
/// state moves with the units (sent units first), so neither bill fires or
/// cancels anything by being split. A percentage bill discount applies to
/// both bills; a baht bill discount stays on the original. The new bill has
/// no table. At least one unit must move and one must stay.
pub fn split_cart(
    cart: &Cart,
    take: &[(usize, u32)],
    new_id: Uuid,
) -> Result<(Cart, Cart), DomainError> {
    let mut rest = cart.clone();
    let mut moved = Cart {
        id: new_id,
        label: cart.label.clone(),
        lines: Vec::new(),
        discount: match cart.discount {
            Some(d @ crate::Discount::Percent(_)) => Some(d),
            _ => None,
        },
        discount_reason: String::new(),
        service_charge: cart.service_charge,
        table: None,
        guests: 0,
        sent: Vec::new(),
        coupons: cart.coupons.clone(),
    };
    let mut seen = std::collections::HashSet::new();
    for &(index, quantity) in take {
        let line = rest
            .lines
            .get_mut(index)
            .ok_or(DomainError::InvalidBillSplit)?;
        if quantity == 0 || quantity > line.quantity || !seen.insert(index) {
            return Err(DomainError::InvalidBillSplit);
        }
        line.quantity -= quantity;
        let mut part = line.clone();
        part.quantity = quantity;
        moved.lines.push(part);
    }
    rest.lines.retain(|l| l.quantity > 0);
    if moved.lines.is_empty() || rest.lines.is_empty() {
        return Err(DomainError::InvalidBillSplit);
    }
    // Sent units go with the moved units first.
    for part in &moved.lines {
        let Some(sent) = rest.sent.iter_mut().find(|s| {
            s.product_id == part.product_id && s.options == part.options && s.note == part.note
        }) else {
            continue;
        };
        let units = sent.quantity.min(part.quantity);
        if units == 0 {
            continue;
        }
        sent.quantity -= units;
        match moved.sent.iter_mut().find(|s| {
            s.product_id == part.product_id && s.options == part.options && s.note == part.note
        }) {
            Some(s) => s.quantity += units,
            None => moved.sent.push(crate::SentItem {
                product_id: part.product_id,
                options: part.options.clone(),
                note: part.note.clone(),
                quantity: units,
            }),
        }
    }
    rest.sent.retain(|s| s.quantity > 0);
    if moved.has_discount() {
        moved.discount_reason = cart.discount_reason.clone();
    }
    Ok((rest, moved))
}

/// Immutable snapshots preserve sold names, SKUs and prices after catalog edits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaleLine {
    pub product_id: Uuid,
    pub sku: String,
    pub name: String,
    pub unit_price: Money,
    /// Catalog standard cost at payment, never taken from the cart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_cost: Option<Money>,
    /// Exact allocated share of the receipt VAT for profit reporting.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub vat_share: Money,
    pub quantity: u32,
    pub note: String,
    pub total: Money,
    /// None only on receipts created before stock snapshots were introduced.
    #[serde(default)]
    pub stock_tracked: Option<bool>,
    /// Options included in `unit_price`. Absent on receipts without options.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<crate::SaleOption>,
    /// The line's own discount rule and amount. `total` stays before discounts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discount_rule: Option<crate::Discount>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub discount: Money,
    /// This line's share of the bill discount.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub bill_discount: Money,
    /// Service charge, VAT added on top and cash rounding shares.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub service: Money,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub vat_added: Money,
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub rounding: i64,
    /// Promotion that gave this line's discount, by name at the time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promotion: Option<String>,
}
fn is_zero_u32(n: &u32) -> bool {
    *n == 0
}
fn is_zero_i64(n: &i64) -> bool {
    *n == 0
}
fn is_zero(m: &Money) -> bool {
    m.satang() == 0
}
impl SaleLine {
    /// Revenue returned with these units, excluding this receipt's allocated VAT.
    /// Cumulative paid-value allocation avoids VAT drift across repeated refunds.
    pub fn refund_revenue(&self, done: u32, quantity: u32) -> Result<Money, DomainError> {
        let net = self.net();
        let amount = crate::refund_share(net, self.quantity, done, quantity)?;
        if self.vat_share > net {
            return Err(DomainError::InvalidMoney);
        }
        if net == Money::ZERO {
            return Ok(Money::ZERO);
        }
        let before = crate::refund_share(net, self.quantity, 0, done)?;
        let after = before.checked_add(amount)?;
        let vat_at = |paid: Money| {
            i128::from(self.vat_share.satang()) * i128::from(paid.satang())
                / i128::from(net.satang())
        };
        let vat =
            i64::try_from(vat_at(after) - vat_at(before)).map_err(|_| DomainError::Overflow)?;
        Money::from_satang(amount.satang() - vat)
    }
    /// What the customer paid for this line after all discounts.
    pub fn net(&self) -> Money {
        Money::from_satang(
            self.total.satang() - self.discount.satang() - self.bill_discount.satang()
                + self.service.satang()
                + self.vat_added.satang()
                + self.rounding,
        )
        .unwrap_or_default()
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sale {
    pub id: Uuid,
    pub number: u64,
    pub created_at: i64,
    pub label: String,
    pub lines: Vec<SaleLine>,
    pub total: Money,
    #[serde(default)]
    pub payment_method: crate::PaymentMethod,
    /// Split payments only: the part paid by each method. `payment_method` is
    /// then the first non-cash part. Empty for single-method receipts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tenders: Vec<crate::Tender>,
    /// Cash handed over (whole bill, or the cash part of a split); zero
    /// without cash. `change` is returned from it.
    pub cash: Money,
    pub change: Money,
    /// Bill discount rule and amount (spread over lines as `bill_discount`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discount_rule: Option<crate::Discount>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub discount: Money,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub discount_reason: String,
    /// Service charge (rate in basis points), VAT added on top of exclusive
    /// prices, and cash rounding in satang. `total` already includes them.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub service_charge: Money,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub service_rate_bp: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub vat_added: Money,
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub rounding: i64,
    /// VAT snapshot when the shop was VAT-registered at payment time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tax: Option<crate::SaleTax>,
    /// Who did it and who approved (empty when the shop has no staff).
    #[serde(default, skip_serializing_if = "crate::Attribution::is_empty")]
    pub staff: crate::Attribution,
    /// Promotions used on this bill and what each took off.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub promotions: Vec<crate::PromoUse>,
}
impl Sale {
    /// What each method paid: the split parts, or the whole total.
    pub fn paid_by(&self) -> Vec<crate::Tender> {
        if self.tenders.is_empty() {
            vec![crate::Tender {
                method: self.payment_method,
                amount: self.total,
            }]
        } else {
            self.tenders.clone()
        }
    }
    pub fn is_split(&self) -> bool {
        !self.tenders.is_empty()
    }
    /// Distribute VAT exactly by paid line amounts (largest remainder).
    /// Used once at checkout; each resulting share is an immutable snapshot.
    pub fn allocate_vat(&mut self) -> Result<(), DomainError> {
        let vat = self.tax.as_ref().map_or(Money::ZERO, |tax| tax.vat);
        let weights: Vec<_> = self.lines.iter().map(|line| line.net().satang()).collect();
        for (line, share) in self
            .lines
            .iter_mut()
            .zip(crate::discount::allocate(vat, &weights)?)
        {
            line.vat_share = share;
        }
        Ok(())
    }
    /// Sum of lines before any discount.
    pub fn subtotal(&self) -> Money {
        Money::from_satang(self.lines.iter().map(|l| l.total.satang()).sum()).unwrap_or_default()
    }
    /// Line and bill discounts together.
    pub fn discounts(&self) -> Money {
        Money::from_satang(
            self.lines.iter().map(|l| l.discount.satang()).sum::<i64>() + self.discount.satang(),
        )
        .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reject_empty_cart_and_zero_quantity() {
        let mut cart = Cart::new();
        assert_eq!(cart.validate(), Err(DomainError::EmptyCart));
        cart.lines.push(CartLine {
            product_id: Uuid::new_v4(),
            quantity: 0,
            note: String::new(),
            ..Default::default()
        });
        assert_eq!(cart.validate(), Err(DomainError::InvalidQuantity));
    }
    #[test]
    fn splitting_a_bill_moves_units_and_their_kitchen_state() -> Result<(), DomainError> {
        let (a, b) = (Uuid::from_u128(1), Uuid::from_u128(2));
        let line = |p, q| CartLine {
            product_id: p,
            quantity: q,
            ..Default::default()
        };
        let mut cart = Cart::new();
        cart.table = Some(Uuid::from_u128(9));
        cart.guests = 4;
        cart.lines = vec![line(a, 3), line(b, 1)];
        cart.discount = Some(crate::Discount::Percent(1000));
        // Two of the three A are with the kitchen, B is not sent yet.
        cart.sent = vec![crate::SentItem {
            product_id: a,
            options: vec![],
            note: String::new(),
            quantity: 2,
        }];
        let kitchen = |_| true;
        let before: i64 = crate::kitchen_changes(&cart, kitchen)
            .0
            .iter()
            .map(|c| c.1)
            .sum();
        let (rest, moved) = split_cart(&cart, &[(0, 2)], Uuid::from_u128(5))?;
        assert_eq!(rest.lines, vec![line(a, 1), line(b, 1)]);
        assert_eq!(moved.lines, vec![line(a, 2)]);
        assert_eq!((moved.table, moved.guests), (None, 0));
        assert_eq!(moved.discount, cart.discount);
        // Nothing fires or cancels because of the split.
        let after: i64 = [&rest, &moved]
            .iter()
            .map(|c| {
                crate::kitchen_changes(c, kitchen)
                    .0
                    .iter()
                    .map(|c| c.1)
                    .sum::<i64>()
            })
            .sum();
        assert_eq!(before, after);
        assert!(crate::kitchen_changes(&moved, kitchen).0.is_empty());
        // Everything or nothing cannot be split off.
        assert_eq!(
            split_cart(&cart, &[(0, 3), (1, 1)], Uuid::nil()).err(),
            Some(DomainError::InvalidBillSplit)
        );
        assert_eq!(
            split_cart(&cart, &[(0, 4)], Uuid::nil()).err(),
            Some(DomainError::InvalidBillSplit)
        );
        Ok(())
    }
    #[test]
    fn split_payments_merge_per_method_and_need_two_methods() -> Result<(), DomainError> {
        use crate::{PaymentMethod as M, SplitPayment, Tender};
        let baht = |b: i64| Money::from_satang(b * 100);
        let split = SplitPayment {
            tenders: vec![
                Tender {
                    method: M::Transfer,
                    amount: baht(100)?,
                },
                Tender {
                    method: M::Cash,
                    amount: baht(50)?,
                },
                Tender {
                    method: M::Transfer,
                    amount: baht(20)?,
                },
            ],
            cash_given: baht(100)?,
        };
        let parts = split.normalized()?;
        assert_eq!(
            parts[0],
            Tender {
                method: M::Cash,
                amount: baht(50)?
            }
        );
        assert_eq!(
            parts[1],
            Tender {
                method: M::Transfer,
                amount: baht(120)?
            }
        );
        assert_eq!(SplitPayment::total(&parts)?, baht(170)?);
        let one = SplitPayment {
            tenders: vec![Tender {
                method: M::Card,
                amount: baht(10)?,
            }],
            cash_given: Money::ZERO,
        };
        assert_eq!(one.normalized().err(), Some(DomainError::InvalidSplit));
        let short = SplitPayment {
            cash_given: baht(10)?,
            ..split
        };
        assert_eq!(
            short.normalized().err(),
            Some(DomainError::InsufficientPayment)
        );
        Ok(())
    }
    #[test]
    fn separate_lines_allow_different_food_notes() {
        let id = Uuid::new_v4();
        let cart = Cart {
            id: Uuid::new_v4(),
            label: "กลับบ้าน".into(),
            lines: vec![
                CartLine {
                    product_id: id,
                    quantity: 1,
                    note: "ไม่ใส่พริก".into(),
                    ..Default::default()
                },
                CartLine {
                    product_id: id,
                    quantity: 1,
                    note: "เผ็ดมาก".into(),
                    ..Default::default()
                },
            ],
            ..Cart::new()
        };
        assert!(cart.validate().is_ok());
    }
}
