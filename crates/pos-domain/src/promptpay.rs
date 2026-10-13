//! PromptPay QR payloads (Thai QR Payment, EMVCo merchant-presented mode).
//! Built offline from the shop's PromptPay ID; the customer's banking app
//! reads the amount from the code. The app never checks whether money arrived.
use crate::{DomainError, Money};

/// Proxy type of a PromptPay ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptPayKind {
    /// Mobile number, 10 digits starting with 0.
    Phone,
    /// National ID or tax ID, 13 digits.
    TaxId,
    /// E-wallet ID, 15 digits.
    EWallet,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptPayId {
    kind: PromptPayKind,
    digits: String,
}
impl PromptPayId {
    /// Accepts digits with optional spaces or dashes ("081-234-5678").
    pub fn parse(text: &str) -> Result<Self, DomainError> {
        let mut digits = String::new();
        for c in text.trim().chars() {
            match c {
                '0'..='9' => digits.push(c),
                ' ' | '-' => {}
                _ => return Err(DomainError::InvalidPromptPay),
            }
        }
        let kind = match digits.len() {
            10 if digits.starts_with('0') => PromptPayKind::Phone,
            13 => PromptPayKind::TaxId,
            15 => PromptPayKind::EWallet,
            _ => return Err(DomainError::InvalidPromptPay),
        };
        Ok(Self { kind, digits })
    }
    pub fn kind(&self) -> PromptPayKind {
        self.kind
    }
    /// Normalised digits, as stored.
    pub fn digits(&self) -> &str {
        &self.digits
    }
    /// Shown beside the QR so the customer can check the payee:
    /// "08x-xxx-5678" for phones, last four digits otherwise.
    pub fn masked(&self) -> String {
        let tail = &self.digits[self.digits.len() - 4..];
        match self.kind {
            PromptPayKind::Phone => format!("{}x-xxx-{tail}", &self.digits[..2]),
            _ => format!("x-xxxx-xxxxx-{tail}"),
        }
    }
    /// EMVCo payload. With an amount the code is single-use ("dynamic") and
    /// the customer's app pre-fills the exact total.
    pub fn payload(&self, amount: Option<Money>) -> String {
        let (tag, account) = match self.kind {
            // Phones are sent in international form without the leading 0.
            PromptPayKind::Phone => ("01", format!("0066{}", &self.digits[1..])),
            PromptPayKind::TaxId => ("02", self.digits.clone()),
            PromptPayKind::EWallet => ("03", self.digits.clone()),
        };
        let merchant = format!(
            "{}{}",
            field("00", "A000000677010111"),
            field(tag, &account)
        );
        let amount = amount.filter(|a| a.satang() > 0);
        let mut out = String::new();
        out.push_str(&field("00", "01"));
        out.push_str(&field("01", if amount.is_some() { "12" } else { "11" }));
        out.push_str(&field("29", &merchant));
        // Field order follows the widely used Thai QR generators.
        out.push_str(&field("58", "TH"));
        out.push_str(&field("53", "764"));
        if let Some(a) = amount {
            out.push_str(&field("54", &a.to_string()));
        }
        out.push_str("6304");
        let crc = crc16(out.as_bytes());
        out.push_str(&format!("{crc:04X}"));
        out
    }
}
fn field(id: &str, value: &str) -> String {
    format!("{id}{:02}{value}", value.len())
}
/// CRC-16/CCITT-FALSE (poly 0x1021, init 0xFFFF), as required by EMVCo.
fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        crc ^= u16::from(byte) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crc_matches_the_standard_check_value() {
        assert_eq!(crc16(b"123456789"), 0x29B1);
    }
    #[test]
    fn ids_parse_by_length() -> Result<(), DomainError> {
        assert_eq!(
            PromptPayId::parse("081-234-5678")?.kind(),
            PromptPayKind::Phone
        );
        assert_eq!(
            PromptPayId::parse("1 2345 67890 12 3")?.kind(),
            PromptPayKind::TaxId
        );
        assert_eq!(
            PromptPayId::parse("123456789012345")?.kind(),
            PromptPayKind::EWallet
        );
        for bad in ["", "812345678", "1812345678", "08123456789", "08a2345678"] {
            assert_eq!(
                PromptPayId::parse(bad),
                Err(DomainError::InvalidPromptPay),
                "{bad}"
            );
        }
        assert_eq!(PromptPayId::parse("0812345678")?.masked(), "08x-xxx-5678");
        Ok(())
    }
    #[test]
    fn payloads_follow_thai_qr_layout() -> Result<(), DomainError> {
        let phone = PromptPayId::parse("0812345678")?;
        // Reference value from the open-source promptpay-qr generator.
        assert_eq!(
            PromptPayId::parse("000-000-0000")?.payload(None),
            "00020101021129370016A000000677010111011300660000000005802TH530376463048956"
        );
        let amount: Money = "1250.5".parse()?;
        let dynamic = phone.payload(Some(amount));
        assert!(dynamic.contains("010212"));
        assert!(dynamic.contains("54071250.50"));
        // The CRC covers everything before it, including "6304".
        let (body, crc) = dynamic.split_at(dynamic.len() - 4);
        assert_eq!(crc, format!("{:04X}", crc16(body.as_bytes())));
        let tax = PromptPayId::parse("1234567890123")?.payload(None);
        assert!(tax.contains("29370016A00000067701011102131234567890123"));
        // Zero amounts fall back to a static code.
        assert!(phone.payload(Some(Money::ZERO)).contains("010211"));
        Ok(())
    }
}
