//! VAT settings, full tax invoices and credit notes. Receipts snapshot the
//! seller's tax details at payment (see checkout); invoices are numbered
//! separately and issued at most once per sale; refunds and voids of an
//! invoiced sale issue a credit note in the same transaction.
use crate::{
    Error, Pos, Result,
    schema::{app_settings, credit_notes, sale_refunds, sale_voids, sales, tax_invoices},
};
use diesel::prelude::*;
use pos_domain::{Buyer, CreditLine, CreditNote, Money, Permission, Sale, TaxInvoice, TaxSettings};
use uuid::Uuid;

/// What a refund or void returned, for its credit note.
pub(crate) struct Credit<'a> {
    pub source_id: Uuid,
    pub reason: &'a str,
    pub lines: Vec<CreditLine>,
    /// Refunded amount, VAT included.
    pub total: Money,
    /// VAT within `total`.
    pub vat: Money,
    pub staff: &'a pos_domain::Attribution,
}
/// Issues the credit note for a refund or void of `sale` when it has a full
/// tax invoice; otherwise nothing. The caller owns the IMMEDIATE transaction.
pub(crate) fn credit_note(
    conn: &mut SqliteConnection,
    sale: &Sale,
    credit: Credit<'_>,
    now: i64,
) -> Result<Option<CreditNote>> {
    let Some(json) = tax_invoices::table
        .filter(tax_invoices::sale_id.eq(sale.id.to_string()))
        .select(tax_invoices::invoice_json)
        .first::<String>(conn)
        .optional()?
    else {
        return Ok(None);
    };
    let invoice: TaxInvoice = serde_json::from_str(&json)?;
    let seller = sale.tax.clone().ok_or(Error::CorruptData)?;
    let mut credited = Money::ZERO;
    for json in credit_notes::table
        .filter(credit_notes::sale_id.eq(sale.id.to_string()))
        .select(credit_notes::note_json)
        .load::<String>(conn)?
    {
        let earlier: CreditNote = serde_json::from_str(&json)?;
        credited = credited.checked_add(earlier.difference)?;
    }
    let difference = Money::from_satang(credit.total.satang() - credit.vat.satang())?;
    let after = credited.checked_add(difference)?;
    // Rounding across many partial refunds can leave a satang over the base.
    let correct_value = Money::from_satang((seller.base.satang() - after.satang()).max(0))?;
    let id = Uuid::new_v4();
    let number = diesel::insert_into(credit_notes::table)
        .values((
            credit_notes::id.eq(id.to_string()),
            credit_notes::sale_id.eq(sale.id.to_string()),
            credit_notes::source_id.eq(credit.source_id.to_string()),
            credit_notes::created_at.eq(now),
            credit_notes::note_json.eq("{}"),
        ))
        .returning(credit_notes::number)
        .get_result::<i64>(conn)?;
    let note = CreditNote {
        id,
        number: u64::try_from(number).map_err(|_| Error::CorruptData)?,
        sale_id: sale.id,
        sale_number: sale.number,
        invoice_number: invoice.number,
        invoice_date: invoice.created_at,
        source_id: credit.source_id,
        created_at: now,
        reason: credit.reason.to_owned(),
        original_value: seller.base,
        seller,
        buyer: invoice.buyer,
        lines: credit.lines,
        correct_value,
        difference,
        vat: credit.vat,
        total: credit.total,
        staff: credit.staff.clone(),
    };
    diesel::update(credit_notes::table.find(number))
        .set(credit_notes::note_json.eq(serde_json::to_string(&note)?))
        .execute(conn)?;
    Ok(Some(note))
}

const TAX_KEY: &str = "tax";
const PRICING_KEY: &str = "pricing";

pub(crate) fn pricing(conn: &mut SqliteConnection) -> Result<pos_domain::PricingSettings> {
    match app_settings::table
        .find(PRICING_KEY)
        .select(app_settings::value)
        .first::<String>(conn)
        .optional()?
    {
        Some(json) => Ok(serde_json::from_str(&json)?),
        None => Ok(pos_domain::PricingSettings::default()),
    }
}

pub(crate) fn settings(conn: &mut SqliteConnection) -> Result<TaxSettings> {
    match app_settings::table
        .find(TAX_KEY)
        .select(app_settings::value)
        .first::<String>(conn)
        .optional()?
    {
        Some(json) => Ok(serde_json::from_str(&json)?),
        None => Ok(TaxSettings::default()),
    }
}
impl Pos {
    pub fn tax_settings(&mut self) -> Result<TaxSettings> {
        settings(&mut self.conn)
    }
    pub fn pricing_settings(&mut self) -> Result<pos_domain::PricingSettings> {
        pricing(&mut self.conn)
    }
    /// Service charge and cash rounding. Owner-level; applies to later sales.
    pub fn set_pricing_settings(&mut self, value: &pos_domain::PricingSettings) -> Result<()> {
        value.validate()?;
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            crate::staff::authorize(conn, &session, Some(Permission::Settings))?;
            diesel::replace_into(app_settings::table)
                .values((
                    app_settings::key.eq(PRICING_KEY),
                    app_settings::value.eq(serde_json::to_string(value)?),
                ))
                .execute(conn)?;
            Ok(())
        })
    }
    /// Owner-level change; applies to receipts paid afterwards only.
    pub fn set_tax_settings(&mut self, value: &TaxSettings) -> Result<()> {
        value.validate()?;
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            crate::staff::authorize(conn, &session, Some(Permission::Settings))?;
            diesel::replace_into(app_settings::table)
                .values((
                    app_settings::key.eq(TAX_KEY),
                    app_settings::value.eq(serde_json::to_string(value)?),
                ))
                .execute(conn)?;
            Ok(())
        })
    }
    /// Issues the full tax invoice for a VAT receipt. Retrying with the same
    /// buyer returns the original invoice; a different buyer is refused.
    /// Voided or refunded sales are refused: the invoice must come first, then
    /// each refund of the invoiced sale issues a credit note.
    pub fn issue_tax_invoice(
        &mut self,
        sale_id: Uuid,
        buyer: &Buyer,
        now: i64,
    ) -> Result<TaxInvoice> {
        buyer.validate()?;
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            if let Some(json) = tax_invoices::table
                .filter(tax_invoices::sale_id.eq(sale_id.to_string()))
                .select(tax_invoices::invoice_json)
                .first::<String>(conn)
                .optional()?
            {
                let existing: TaxInvoice = serde_json::from_str(&json)?;
                let same = existing.buyer.name == buyer.name.trim()
                    && existing.buyer.tax_id == buyer.tax_id
                    && existing.buyer.branch == buyer.branch
                    && existing.buyer.address == buyer.address.trim();
                return if same {
                    Ok(existing)
                } else {
                    Err(Error::ConflictingRequest)
                };
            }
            let staff = crate::staff::authorize(conn, &session, None)?;
            let json = sales::table
                .filter(sales::cart_id.eq(sale_id.to_string()))
                .select(sales::receipt_json)
                .first::<String>(conn)
                .optional()?
                .ok_or(Error::SaleNotFound)?;
            let sale: Sale = serde_json::from_str(&json)?;
            if sale.tax.is_none() {
                return Err(Error::NotTaxReceipt);
            }
            let voided = diesel::select(diesel::dsl::exists(
                sale_voids::table.filter(sale_voids::sale_id.eq(sale_id.to_string())),
            ))
            .get_result::<bool>(conn)?;
            let refunded = diesel::select(diesel::dsl::exists(
                sale_refunds::table.filter(sale_refunds::sale_id.eq(sale_id.to_string())),
            ))
            .get_result::<bool>(conn)?;
            if voided || refunded {
                return Err(Error::SaleRefunded);
            }
            if now < sale.created_at {
                return Err(Error::InvalidVoidTime);
            }
            let id = Uuid::new_v4();
            let number = diesel::insert_into(tax_invoices::table)
                .values((
                    tax_invoices::id.eq(id.to_string()),
                    tax_invoices::sale_id.eq(sale_id.to_string()),
                    tax_invoices::created_at.eq(now),
                    tax_invoices::invoice_json.eq("{}"),
                ))
                .returning(tax_invoices::number)
                .get_result::<i64>(conn)?;
            let invoice = TaxInvoice {
                id,
                number: u64::try_from(number).map_err(|_| Error::CorruptData)?,
                sale_id,
                sale_number: sale.number,
                created_at: now,
                buyer: Buyer {
                    name: buyer.name.trim().to_owned(),
                    tax_id: buyer.tax_id.clone(),
                    branch: buyer.branch.clone(),
                    address: buyer.address.trim().to_owned(),
                },
                staff,
            };
            diesel::update(tax_invoices::table.find(number))
                .set(tax_invoices::invoice_json.eq(serde_json::to_string(&invoice)?))
                .execute(conn)?;
            Ok(invoice)
        })
    }
}
