//! Printable documents built from committed records: receipts, shift
//! summaries and a test page. Amounts use the same formatting as the screen.
pub use super::template::ReceiptTemplate;
use super::{
    render::{Align, Bitmap, Doc, Style},
    template::{self, KitchenTemplate},
};
use crate::{i18n::Language, ui::format};
use pos_domain::{PaymentMethod, SaleRecord, Shift, SignedMoney};

/// Shop identity for document headers.
pub struct Brand<'a> {
    pub shop: &'a str,
    pub logo: Option<&'a Bitmap>,
}
fn header(d: &mut Doc, brand: &Brand, t: &ReceiptTemplate, name: &str) {
    if t.logo
        && let Some(logo) = brand.logo
    {
        d.image(logo.clone());
    }
    d.text(name, Style::Title, Align::Center);
    for line in template::lines(&t.header) {
        d.text(line, Style::Small, Align::Center);
    }
}
fn footer(d: &mut Doc, t: &ReceiptTemplate) {
    let mut first = true;
    for line in template::lines(&t.footer) {
        if first {
            d.space();
            first = false;
        }
        d.text(line, Style::Body, Align::Center);
    }
}

/// A sales receipt laid out by the shop's template. Reprints are marked as
/// copies. VAT receipts keep every legally required line whatever the template.
pub fn receipt(
    record: &SaleRecord,
    brand: &Brand,
    t: &ReceiptTemplate,
    app_lang: Language,
    copy: bool,
) -> Doc {
    let lang = t.language.resolve(app_lang);
    let sale = &record.sale;
    let mut d = Doc {
        size: t.size,
        ..Doc::default()
    };
    match &sale.tax {
        Some(tax) => {
            header(&mut d, brand, t, &tax.seller_name);
            if tax.seller_name != brand.shop {
                d.text(brand.shop, Style::Small, Align::Center);
            }
            d.text(tax.address.clone(), Style::Small, Align::Center);
            d.text(
                format!(
                    "{} {} · {}",
                    lang.text("เลขประจำตัวผู้เสียภาษี", "Tax ID"),
                    tax.tax_id,
                    crate::tax_ui::branch_label(lang, &tax.branch)
                ),
                Style::Small,
                Align::Center,
            );
            if !tax.pos_id.is_empty() {
                d.text(
                    format!("POS ID {}", tax.pos_id),
                    Style::Small,
                    Align::Center,
                );
            }
            d.text(
                lang.text(
                    "ใบกำกับภาษีอย่างย่อ / ใบเสร็จรับเงิน",
                    "Abbreviated tax invoice / receipt",
                ),
                Style::Strong,
                Align::Center,
            );
            d.text("(VAT Included)", Style::Small, Align::Center);
        }
        None => {
            header(&mut d, brand, t, brand.shop);
            if t.title.trim().is_empty() {
                d.text(
                    lang.text("ใบเสร็จรับเงิน · ไม่ใช่ใบกำกับภาษี", "Receipt · not a tax invoice"),
                    Style::Small,
                    Align::Center,
                );
            } else {
                d.text(t.title.trim(), Style::Strong, Align::Center);
                d.text(
                    lang.text("ไม่ใช่ใบกำกับภาษี", "Not a tax invoice"),
                    Style::Small,
                    Align::Center,
                );
            }
        }
    }
    if copy {
        d.text(
            lang.text("— สำเนา —", "— COPY —"),
            Style::Strong,
            Align::Center,
        );
    }
    d.space();
    d.row(
        format!(
            "{} {}",
            lang.text("บิล", "Receipt"),
            format::receipt_number(sale.number)
        ),
        "",
        Style::Strong,
    );
    d.text(
        format::date_time(lang, sale.created_at),
        Style::Small,
        Align::Left,
    );
    if t.show_label && !sale.label.is_empty() {
        d.text(sale.label.clone(), Style::Small, Align::Left);
    }
    if t.show_staff
        && let Some(by) = &sale.staff.by
    {
        d.text(
            format!("{} {}", lang.text("พนักงาน", "Staff"), by.name),
            Style::Small,
            Align::Left,
        );
    }
    d.divider();
    for line in &sale.lines {
        d.row(
            format!(
                "{} × {}",
                format::count(u64::from(line.quantity)),
                line.name
            ),
            format::money(line.total),
            Style::Body,
        );
        if t.show_sku && !line.sku.is_empty() {
            d.detail(line.sku.clone(), "");
        }
        if t.show_options {
            for o in &line.options {
                let text = if o.price.satang() > 0 {
                    format!("{} +{}", o.name, format::money(o.price))
                } else {
                    o.name.clone()
                };
                d.detail(text, "");
            }
        }
        if t.show_notes && !line.note.is_empty() {
            d.detail(line.note.clone(), "");
        }
        if t.show_unit_price && line.quantity > 1 {
            d.detail(format!("@ {}", format::money(line.unit_price)), "");
        }
        if line.discount.satang() > 0 {
            let rule = line.discount_rule.map_or_else(
                || lang.text("ส่วนลด", "Discount").to_owned(),
                |r| crate::discount_ui::label(lang, r),
            );
            d.detail(rule, format::minus(line.discount));
        }
    }
    d.divider();
    let extras =
        sale.service_charge.satang() > 0 || sale.vat_added.satang() > 0 || sale.rounding != 0;
    if sale.discounts().satang() > 0 || extras {
        d.row(
            lang.text("รวมรายการ", "Items total"),
            format::money(sale.subtotal()),
            Style::Body,
        );
    }
    if sale.discount.satang() > 0 {
        let label = match sale.discount_rule {
            Some(rule) => format!(
                "{} {}",
                lang.text("ส่วนลดทั้งบิล", "Bill discount"),
                format::discount_rule(rule)
            ),
            None => lang.text("ส่วนลดทั้งบิล", "Bill discount").to_owned(),
        };
        d.row(label, format::minus(sale.discount), Style::Body);
    }
    for (label, value) in crate::history_ui::pricing_rows(lang, sale) {
        d.row(label, value, Style::Body);
    }
    d.row(
        lang.text("ยอดรวม", "Total"),
        format::baht(sale.total),
        Style::Total,
    );
    if let Some(tax) = &sale.tax {
        d.detail(
            lang.text("มูลค่าสินค้า", "Value before VAT"),
            format::money(tax.base),
        );
        d.detail(
            format!("VAT {}", crate::tax_ui::rate_label(tax.rate_bp)),
            format::money(tax.vat),
        );
    }
    match sale.payment_method {
        PaymentMethod::Cash if t.show_cash => {
            d.row(
                lang.text("รับเงินสด", "Cash"),
                format::money(sale.cash),
                Style::Body,
            );
            d.row(
                lang.text("เงินทอน", "Change"),
                format::money(sale.change),
                Style::Body,
            );
        }
        method => d.row(
            lang.text("ชำระโดย", "Paid by"),
            lang.payment_method(method),
            Style::Body,
        ),
    }
    for r in &record.refunds {
        d.detail(
            format!(
                "{} {}",
                lang.text("คืนบางส่วน", "Partial refund"),
                format::date_time(lang, r.created_at)
            ),
            format::minus(r.refund),
        );
    }
    if let Some(c) = &record.cancellation {
        d.detail(
            format!(
                "{} {}",
                lang.text("ยกเลิกบิล คืนเงิน", "Voided, refunded"),
                format::date_time(lang, c.created_at)
            ),
            format::minus(c.refund),
        );
    }
    footer(&mut d, t);
    d
}

/// One line of a check bill (before payment).
pub struct BillLine {
    pub name: String,
    pub quantity: u32,
    pub amount: pos_domain::Money,
    pub options: Vec<String>,
}
/// Check bill (ใบแจ้งยอด): what the table owes before paying, with an
/// optional PromptPay QR for the exact amount. Not a receipt.
#[allow(clippy::too_many_arguments)]
pub fn check_bill(
    lines: &[BillLine],
    price: &pos_domain::BillPrice,
    cash_total: Option<pos_domain::Money>,
    label: &str,
    brand: &Brand,
    t: &ReceiptTemplate,
    promptpay: Option<String>,
    app_lang: Language,
) -> Doc {
    let lang = t.language.resolve(app_lang);
    let mut d = Doc {
        size: t.size,
        ..Doc::default()
    };
    header(&mut d, brand, t, brand.shop);
    d.text(
        lang.text("ใบแจ้งยอด · ยังไม่ใช่ใบเสร็จ", "Check bill · not a receipt"),
        Style::Strong,
        Align::Center,
    );
    if !label.is_empty() {
        d.text(label, Style::Title, Align::Center);
    }
    d.text(
        format::date_time(lang, time::OffsetDateTime::now_utc().unix_timestamp()),
        Style::Small,
        Align::Center,
    );
    d.divider();
    for line in lines {
        d.row(
            format!("{} × {}", line.quantity, line.name),
            format::money(line.amount),
            Style::Body,
        );
        if t.show_options && !line.options.is_empty() {
            d.detail(line.options.join(" · "), "");
        }
    }
    d.divider();
    if price.discounts().satang() > 0
        || price.service_charge.satang() > 0
        || price.vat_added.satang() > 0
    {
        d.row(
            lang.text("รวมรายการ", "Items total"),
            format::money(price.subtotal),
            Style::Body,
        );
    }
    if price.discounts().satang() > 0 {
        d.row(
            lang.text("ส่วนลด", "Discount"),
            format::minus(price.discounts()),
            Style::Body,
        );
    }
    if price.service_charge.satang() > 0 {
        d.row(
            "Service charge",
            format::money(price.service_charge),
            Style::Body,
        );
    }
    if price.vat_added.satang() > 0 {
        d.row("VAT", format::money(price.vat_added), Style::Body);
    }
    d.row(
        lang.text("ยอดชำระ", "Amount due"),
        format::baht(price.total),
        Style::Total,
    );
    if let Some(cash) = cash_total.filter(|c| *c != price.total) {
        d.detail(
            lang.text("ถ้าจ่ายเงินสด (ปัดเศษ)", "If paid in cash (rounded)"),
            format::money(cash),
        );
    }
    if t.bill_qr
        && let Some(payload) = promptpay
    {
        d.space();
        d.text(
            lang.text("สแกนจ่ายด้วยพร้อมเพย์", "Scan to pay with PromptPay"),
            Style::Small,
            Align::Center,
        );
        d.qr(payload);
    }
    footer(&mut d, t);
    d
}

/// End-of-shift summary: drawer reconciliation and sales by method.
pub fn shift_summary(shift: &Shift, number: Option<u64>, shop: &str, lang: Language) -> Doc {
    let mut d = Doc::default();
    let m = |v| format::money(v);
    d.text(shop, Style::Title, Align::Center);
    d.text(
        match number {
            Some(n) => format!("{} {n}", lang.text("สรุปกะที่", "Shift summary #")),
            None => lang.text("สรุปกะ", "Shift summary").to_owned(),
        },
        Style::Strong,
        Align::Center,
    );
    d.space();
    d.row(
        lang.text("เปิดกะ", "Opened"),
        format::date_time(lang, shift.opened_at),
        Style::Small,
    );
    if let Some(by) = &shift.opened_by {
        d.row(
            lang.text("เปิดโดย", "Opened by"),
            by.name.clone(),
            Style::Small,
        );
    }
    if let Some(c) = &shift.closing {
        d.row(
            lang.text("ปิดกะ", "Closed"),
            format::date_time(lang, c.closed_at),
            Style::Small,
        );
        if let Some(by) = &c.closed_by {
            d.row(
                lang.text("ปิดโดย", "Closed by"),
                by.name.clone(),
                Style::Small,
            );
        }
    }
    d.divider();
    d.text(
        lang.text("เงินสดในลิ้นชัก", "Drawer cash"),
        Style::Strong,
        Align::Left,
    );
    d.row(
        lang.text("เงินตั้งต้น", "Opening float"),
        m(shift.opening),
        Style::Body,
    );
    d.row(
        lang.text("ขายเงินสด", "Cash sales"),
        m(shift.sales),
        Style::Body,
    );
    d.row(
        lang.text("คืนเงินสด", "Cash refunds"),
        format::minus(shift.refunds),
        Style::Body,
    );
    d.row(
        lang.text("นำเงินเข้า", "Cash in"),
        m(shift.cash_in),
        Style::Body,
    );
    d.row(
        lang.text("นำเงินออก", "Cash out"),
        format::minus(shift.cash_out),
        Style::Body,
    );
    if let Ok(expected) = shift.expected() {
        d.row(
            lang.text("ควรมีในลิ้นชัก", "Expected"),
            format::signed(expected),
            Style::Strong,
        );
    }
    if let Some(c) = &shift.closing {
        d.row(lang.text("นับได้", "Counted"), m(c.counted), Style::Strong);
        d.row(
            lang.text("ส่วนต่าง", "Difference"),
            format::delta(c.difference),
            Style::Strong,
        );
        if !c.reason.is_empty() {
            d.detail(c.reason.clone(), "");
        }
        for count in &c.denominations {
            if count.count > 0 {
                d.detail(
                    format!("{} × {}", format::money(count.value), count.count),
                    m(count.value.checked_mul(count.count).unwrap_or_default()),
                );
            }
        }
    }
    d.divider();
    d.text(
        lang.text("ยอดขายตามวิธีชำระ", "Sales by method"),
        Style::Strong,
        Align::Left,
    );
    for (method, sales, refunds) in [
        (PaymentMethod::Cash, shift.sales, shift.refunds),
        (
            PaymentMethod::Transfer,
            shift.transfer_sales,
            shift.transfer_refunds,
        ),
        (PaymentMethod::Card, shift.card_sales, shift.card_refunds),
    ] {
        let net = SignedMoney::from_satang(sales.satang() - refunds.satang());
        d.row(
            lang.payment_method(method),
            format::signed(net),
            Style::Body,
        );
        if refunds.satang() > 0 {
            d.detail(
                format!("{} {}", lang.text("ขาย", "sales"), m(sales)),
                format::minus(refunds),
            );
        }
    }
    d.row(
        lang.text("จำนวนบิล", "Receipts"),
        format::count(shift.sale_count),
        Style::Body,
    );
    d.row(
        lang.text("จำนวนคืนเงิน", "Refunds"),
        format::count(shift.refund_count),
        Style::Body,
    );
    d.space();
    d.text(
        lang.text(
            "ยอดเงินโอนและบัตรไม่อยู่ในลิ้นชัก",
            "Transfers and cards are not in the drawer",
        ),
        Style::Small,
        Align::Center,
    );
    d
}

/// Printer check: width ruler, Thai text and both number styles.
pub fn test_page(shop: &str, paper: &str, lang: Language) -> Doc {
    let mut d = Doc::default();
    d.text(shop, Style::Title, Align::Center);
    d.text(
        format!("{} · {paper}", lang.text("ทดสอบเครื่องพิมพ์", "Printer test")),
        Style::Strong,
        Align::Center,
    );
    d.divider();
    d.text(
        "ภาษาไทย: กินข้าวหรือยัง ผู้ใหญ่ ฤๅษี น้ำแข็งใส",
        Style::Body,
        Align::Left,
    );
    d.text(
        "English: The quick brown fox 0123456789",
        Style::Body,
        Align::Left,
    );
    d.row(
        lang.text("ตัวอย่างยอด", "Sample amount"),
        "1,234.50",
        Style::Body,
    );
    d.row(lang.text("ยอดรวม", "Total"), "฿1,234.50", Style::Total);
    d.divider();
    d.text(
        lang.text(
            "ถ้าเส้นประเต็มความกว้างกระดาษ ขนาดกระดาษถูกต้อง",
            "If the dashed lines span the paper, the width is right",
        ),
        Style::Small,
        Align::Center,
    );
    d
}

/// Full tax invoice (ใบกำกับภาษีเต็มรูป) for a VAT receipt.
pub fn tax_invoice(
    record: &SaleRecord,
    invoice: &pos_domain::TaxInvoice,
    lang: Language,
    copy: bool,
) -> Option<Doc> {
    let sale = &record.sale;
    let tax = sale.tax.as_ref()?;
    let mut d = Doc::default();
    d.text(
        lang.text("ใบกำกับภาษี / ใบเสร็จรับเงิน", "Tax invoice / receipt"),
        Style::Title,
        Align::Center,
    );
    d.text(
        if copy {
            lang.text("สำเนา", "Copy")
        } else {
            lang.text("ต้นฉบับ", "Original")
        },
        Style::Small,
        Align::Center,
    );
    d.space();
    d.text(tax.seller_name.clone(), Style::Strong, Align::Left);
    d.text(tax.address.clone(), Style::Small, Align::Left);
    d.text(
        format!(
            "{} {} · {}",
            lang.text("เลขประจำตัวผู้เสียภาษี", "Tax ID"),
            tax.tax_id,
            crate::tax_ui::branch_label(lang, &tax.branch)
        ),
        Style::Small,
        Align::Left,
    );
    d.divider();
    d.row(
        lang.text("เลขที่", "No."),
        format!("TI-{:06}", invoice.number),
        Style::Strong,
    );
    d.row(
        lang.text("วันที่", "Date"),
        format::date_time(lang, invoice.created_at),
        Style::Small,
    );
    d.row(
        lang.text("อ้างอิงบิล", "Receipt"),
        format::receipt_number(sale.number),
        Style::Small,
    );
    d.divider();
    d.text(lang.text("ผู้ซื้อ", "Buyer"), Style::Strong, Align::Left);
    d.text(invoice.buyer.name.clone(), Style::Body, Align::Left);
    d.text(invoice.buyer.address.clone(), Style::Small, Align::Left);
    d.text(
        format!(
            "{} {} · {}",
            lang.text("เลขประจำตัวผู้เสียภาษี", "Tax ID"),
            invoice.buyer.tax_id,
            crate::tax_ui::branch_label(lang, &invoice.buyer.branch)
        ),
        Style::Small,
        Align::Left,
    );
    d.divider();
    for line in &sale.lines {
        d.row(
            format!(
                "{} × {}",
                format::count(u64::from(line.quantity)),
                line.name
            ),
            format::money(line.net()),
            Style::Body,
        );
        if line.quantity > 1 {
            d.detail(format!("@ {}", format::money(line.unit_price)), "");
        }
    }
    d.divider();
    d.row(
        lang.text("มูลค่าสินค้า/บริการ", "Value before VAT"),
        format::money(tax.base),
        Style::Body,
    );
    d.row(
        format!("VAT {}", crate::tax_ui::rate_label(tax.rate_bp)),
        format::money(tax.vat),
        Style::Body,
    );
    d.row(
        lang.text("รวมทั้งสิ้น", "Total"),
        format::baht(sale.total),
        Style::Total,
    );
    d.space();
    d.text(
        lang.text(
            "ผู้รับเงิน ____________________",
            "Received by ____________________",
        ),
        Style::Small,
        Align::Left,
    );
    Some(d)
}

/// Kitchen ticket: big table name, dishes with options and notes;
/// cancellations are marked so the kitchen stops cooking them.
pub fn kitchen_ticket(
    ticket: &pos_domain::KitchenTicket,
    t: &KitchenTemplate,
    app_lang: Language,
) -> Doc {
    let lang = t.language.resolve(app_lang);
    let mut d = Doc {
        size: t.size,
        ..Doc::default()
    };
    if ticket.is_cancellation() {
        d.text(
            lang.text("*** ยกเลิก ***", "*** CANCEL ***"),
            Style::Total,
            Align::Center,
        );
    }
    let title = if ticket.table.is_empty() {
        format!("#{}", ticket.number)
    } else {
        ticket.table.clone()
    };
    d.text(title, Style::Total, Align::Center);
    let mut sub = format!(
        "{} #{} · {}",
        lang.text("ใบครัว", "Kitchen"),
        ticket.number,
        format::clock(ticket.created_at)
    );
    if t.show_guests && ticket.guests > 0 {
        sub = format!("{sub} · {} {}", ticket.guests, lang.text("คน", "guests"));
    }
    if t.show_staff
        && let Some(by) = &ticket.staff.by
    {
        sub = format!("{sub} · {}", by.name);
    }
    d.text(sub, Style::Small, Align::Center);
    d.divider();
    for line in &ticket.lines {
        let text = if line.quantity < 0 {
            format!(
                "{} {} × {}",
                lang.text("ยกเลิก", "CANCEL"),
                -line.quantity,
                line.name
            )
        } else {
            format!("{} × {}", line.quantity, line.name)
        };
        d.text(text, Style::Title, Align::Left);
        if t.show_options && !line.options.is_empty() {
            d.detail(line.options.join(" · "), "");
        }
        if !line.note.is_empty() {
            // Notes like "no chilli" must stand out in a hot kitchen.
            d.text(
                format!("   ** {} **", line.note),
                Style::Strong,
                Align::Left,
            );
        }
    }
    d
}
