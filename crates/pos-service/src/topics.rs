//! Change notices (multi-device phase 0d). Every successful write bumps the
//! version of the topics it touches; devices compare versions and reload
//! only what changed. Versions live in the hub process: a device that
//! reconnects simply refreshes everything.
use crate::Request;
use serde::{Deserialize, Serialize};

/// What a screen can depend on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Topic {
    /// Products, categories, option sets, pictures, stock levels.
    Catalog,
    /// Held bills and table bills.
    Bills,
    /// Kitchen tickets.
    Kitchen,
    /// Shifts, drawers and cash movements.
    Shift,
    /// Receipts, refunds, voids, invoices (history and reports).
    Sales,
    /// Staff, roles, tax/pricing settings, tables, promotions, registers.
    Settings,
    /// Ingredients and purchasing.
    Purchasing,
}
impl Topic {
    pub const ALL: [Topic; 7] = [
        Topic::Catalog,
        Topic::Bills,
        Topic::Kitchen,
        Topic::Shift,
        Topic::Sales,
        Topic::Settings,
        Topic::Purchasing,
    ];
}

/// The topics a request changes when it succeeds (empty for reads and
/// sign-in, which only affect the caller).
pub fn changes(request: &Request) -> &'static [Topic] {
    use Topic::*;
    match request {
        Request::Refresh(_)
        | Request::Lookup(_)
        | Request::Activity(_)
        | Request::IngredientHistory(_)
        | Request::LoadPurchases(_)
        | Request::PurchaseDetail(..)
        | Request::PurchaseProducts(_)
        | Request::ProductImage(_)
        | Request::Resume(_)
        | Request::SignIn(..)
        | Request::SignOut => &[],
        Request::SaveProduct(_)
        | Request::Archive(_)
        | Request::SetOnSale(..)
        | Request::SetCategory(..)
        | Request::AdjustStock(_)
        | Request::ImportProducts(_)
        | Request::AddCategory(_)
        | Request::RenameCategory(..)
        | Request::ReorderCategories(_)
        | Request::DeleteCategory(..)
        | Request::SaveOptionSet(_)
        | Request::DeleteOptionSet(_)
        | Request::LinkOptionSet(..)
        | Request::SetProductImage(..) => &[Catalog],
        // A sale moves stock, the drawer, history and removes the held bill.
        Request::Pay(..) | Request::PaySplit(..) => &[Catalog, Bills, Shift, Sales, Purchasing],
        Request::Void(..) | Request::RefundItems(..) => &[Catalog, Shift, Sales, Purchasing],
        Request::SplitBill(..) | Request::Hold(_) | Request::RemoveHeld(_) => &[Bills],
        Request::SendKitchen(_) => &[Bills, Kitchen],
        Request::BumpTicket(..) | Request::TicketLine(..) => &[Kitchen],
        Request::OpenShift(..) | Request::MoveCash(_) | Request::CloseShift(..) => &[Shift],
        Request::IssueInvoice(..) => &[Sales],
        Request::SaveStaff(_)
        | Request::SaveRole(_)
        | Request::DeleteRole(_)
        | Request::SetTax(_)
        | Request::SetPricing(_)
        | Request::SaveTable(_)
        | Request::SavePromotion(_)
        | Request::DeletePromotion(_)
        | Request::AddRegister(..) => &[Settings],
        Request::SaveIngredient(_)
        | Request::AdjustIngredient(_)
        | Request::SaveSupplier(_)
        | Request::CreatePurchase(_)
        | Request::CancelPurchase(_) => &[Purchasing],
        Request::ReceivePurchase(_) => &[Catalog, Purchasing],
        Request::Approved { inner, .. } => changes(inner),
    }
}

/// Current version of every topic (starts at 0, never decreases).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Versions(pub [u64; 7]);
impl Versions {
    fn index(topic: Topic) -> usize {
        Topic::ALL.iter().position(|t| *t == topic).unwrap_or(0)
    }
    pub fn get(&self, topic: Topic) -> u64 {
        self.0[Self::index(topic)]
    }
    /// Records a successful request; returns the topics that changed.
    pub fn record(&mut self, request: &Request) -> &'static [Topic] {
        let topics = changes(request);
        for t in topics {
            let v = &mut self.0[Self::index(*t)];
            *v = v.saturating_add(1);
        }
        topics
    }
    /// Topics whose version differs from an older snapshot.
    pub fn since(&self, seen: &Versions) -> Vec<Topic> {
        Topic::ALL
            .into_iter()
            .filter(|t| self.get(*t) != seen.get(*t))
            .collect()
    }
}
