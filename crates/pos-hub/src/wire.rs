//! Messages between a paired device and the shop hub: length-prefixed JSON
//! frames (4-byte big-endian length, then UTF-8 JSON).
use pos_service::{Reply, Request, Versions};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use uuid::Uuid;

/// Larger frames are refused (product pictures are at most 512 KB).
pub const MAX_FRAME: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    /// First message of every connection.
    Hello {
        protocol: u32,
        device: Uuid,
        token: String,
    },
    /// First message of a pairing connection: the code shown on the hub.
    Pair {
        protocol: u32,
        code: String,
        device: Uuid,
        name: String,
        kind: pos_core::DeviceKind,
    },
    /// One request; `id` is echoed back and is the device's retry key.
    Call { id: Uuid, request: Box<Request> },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    Welcome {
        register: Uuid,
        versions: Versions,
    },
    /// The connection is closed after this.
    Refused(Refusal),
    /// Pairing succeeded: keep `token` secret; it is shown only once.
    Paired {
        token: String,
        register: Uuid,
    },
    Answer {
        id: Uuid,
        result: Result<Box<Reply>, WireError>,
    },
    /// Data changed (by any device); reload the listed topics.
    Changed(Versions),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Refusal {
    /// The device runs another protocol version: update the app.
    Version {
        hub: u32,
    },
    /// Unknown, revoked or wrong token: pair again.
    NotPaired,
    /// Wrong or expired pairing code, or no pairing is open on the hub.
    WrongCode,
    BadMessage,
}

/// A business error as text: the kind for code, the message for people.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireError {
    pub kind: String,
    pub message: String,
    /// Payloads of errors that carry one (approval, stock, schema, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission: Option<pos_domain::Permission>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<pos_domain::DomainError>,
}
impl WireError {
    pub fn new(kind: &str, message: &str) -> Self {
        Self {
            kind: kind.to_owned(),
            message: message.to_owned(),
            permission: None,
            text: None,
            number: None,
            domain: None,
        }
    }
}
impl From<&pos_core::Error> for WireError {
    fn from(e: &pos_core::Error) -> Self {
        use pos_core::Error as E;
        let debug = format!("{e:?}");
        let kind = debug
            .split(|c: char| !c.is_alphanumeric())
            .next()
            .unwrap_or_default()
            .to_owned();
        let mut out = Self::new(&kind, &e.to_string());
        match e {
            E::ApprovalRequired(p) | E::Forbidden(p) => out.permission = Some(*p),
            E::NotForSale(t) | E::InsufficientStock(t) | E::StockTrackingChanged(t) => {
                out.text = Some(t.clone());
            }
            E::UnsupportedSchema(n) => out.number = Some(*n),
            E::PinLocked(n) => out.number = i64::try_from(*n).ok(),
            E::Domain(d) => out.domain = Some(*d),
            E::Remote(m) => out.message = m.clone(),
            _ => {}
        }
        out
    }
}
impl WireError {
    /// The hub's error as the local error type, so devices show the same
    /// messages and approval prompts as the main register.
    pub fn into_core(self) -> pos_core::Error {
        use pos_core::Error as E;
        let text = self.text.clone().unwrap_or_default();
        match self.kind.as_str() {
            "Domain" if self.domain.is_some() => match self.domain {
                Some(d) => E::Domain(d),
                None => E::Remote(self.message),
            },
            "ApprovalRequired" if self.permission.is_some() => match self.permission {
                Some(p) => E::ApprovalRequired(p),
                None => E::Remote(self.message),
            },
            "Forbidden" if self.permission.is_some() => match self.permission {
                Some(p) => E::Forbidden(p),
                None => E::Remote(self.message),
            },
            "NotForSale" => E::NotForSale(text),
            "InsufficientStock" => E::InsufficientStock(text),
            "StockTrackingChanged" => E::StockTrackingChanged(text),
            "UnsupportedSchema" => E::UnsupportedSchema(self.number.unwrap_or_default()),
            "PinLocked" => {
                E::PinLocked(u64::try_from(self.number.unwrap_or_default()).unwrap_or_default())
            }
            "SupplierNotFound" => E::SupplierNotFound,
            "PurchaseOrderNotFound" => E::PurchaseOrderNotFound,
            "PurchaseOrderChanged" => E::PurchaseOrderChanged,
            "PurchaseOrderClosed" => E::PurchaseOrderClosed,
            "PurchaseExceedsOrder" => E::PurchaseExceedsOrder,
            "DuplicateSku" => E::DuplicateSku,
            "DuplicateBarcode" => E::DuplicateBarcode,
            "ProductUnavailable" => E::ProductUnavailable,
            "AlreadyPaid" => E::AlreadyPaid,
            "CartNotFound" => E::CartNotFound,
            "CorruptData" => E::CorruptData,
            "SaleNotFound" => E::SaleNotFound,
            "LegacyStockReviewRequired" => E::LegacyStockReviewRequired,
            "InvalidStockSelection" => E::InvalidStockSelection,
            "InvalidVoidTime" => E::InvalidVoidTime,
            "PaymentTotalChanged" => E::PaymentTotalChanged,
            "InvalidRefundMethod" => E::InvalidRefundMethod,
            "RefundExceedsMethod" => E::RefundExceedsMethod,
            "DuplicateCoupon" => E::DuplicateCoupon,
            "OptionSetNotFound" => E::OptionSetNotFound,
            "InvalidCategory" => E::InvalidCategory,
            "DuplicateCategory" => E::DuplicateCategory,
            "CategoryNotFound" => E::CategoryNotFound,
            "InvalidImage" => E::InvalidImage,
            "IngredientNotFound" => E::IngredientNotFound,
            "DuplicateIngredient" => E::DuplicateIngredient,
            "NoOpenShift" => E::NoOpenShift,
            "ShiftAlreadyOpen" => E::ShiftAlreadyOpen,
            "RegisterNotFound" => E::RegisterNotFound,
            "DeviceNotFound" => E::DeviceNotFound,
            "InvalidDevice" => E::InvalidDevice,
            "InvalidRegister" => E::InvalidRegister,
            "ShiftChanged" => E::ShiftChanged,
            "ShiftClosed" => E::ShiftClosed,
            "InvalidCashMovement" => E::InvalidCashMovement,
            "ConflictingRequest" => E::ConflictingRequest,
            "InvalidShiftTime" => E::InvalidShiftTime,
            "InvalidReportDate" => E::InvalidReportDate,
            "StockChanged" => E::StockChanged,
            "StockNotTracked" => E::StockNotTracked,
            "SaleVoided" => E::SaleVoided,
            "PartiallyRefunded" => E::PartiallyRefunded,
            "RefundExceedsSale" => E::RefundExceedsSale,
            "SignInRequired" => E::SignInRequired,
            "WrongPin" => E::WrongPin,
            "StaffNotFound" => E::StaffNotFound,
            "LastOwner" => E::LastOwner,
            "RoleNotFound" => E::RoleNotFound,
            "RoleInUse" => E::RoleInUse,
            "TableNotFound" => E::TableNotFound,
            "TableBusy" => E::TableBusy,
            "DuplicateTable" => E::DuplicateTable,
            "TicketNotFound" => E::TicketNotFound,
            "NotTaxReceipt" => E::NotTaxReceipt,
            "SaleRefunded" => E::SaleRefunded,
            _ => E::Remote(self.message),
        }
    }
}

#[derive(Debug)]
pub enum FrameError {
    Io(std::io::Error),
    TooLarge(usize),
    Json(serde_json::Error),
}
impl From<std::io::Error> for FrameError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<serde_json::Error> for FrameError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}
impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::TooLarge(n) => write!(f, "frame of {n} bytes is too large"),
            Self::Json(e) => write!(f, "{e}"),
        }
    }
}
impl std::error::Error for FrameError {}

pub fn write_frame<T: Serialize>(out: &mut impl Write, message: &T) -> Result<(), FrameError> {
    let body = serde_json::to_vec(message)?;
    if body.len() > MAX_FRAME {
        return Err(FrameError::TooLarge(body.len()));
    }
    let len = u32::try_from(body.len()).map_err(|_| FrameError::TooLarge(body.len()))?;
    out.write_all(&len.to_be_bytes())?;
    out.write_all(&body)?;
    out.flush()?;
    Ok(())
}
/// Collects bytes from a stream with a read timeout and yields whole
/// frames, so one thread can both wait for messages and send notices.
#[derive(Default)]
pub struct Framer {
    buf: Vec<u8>,
}
impl Framer {
    /// Some(message) when a whole frame arrived; None on a read timeout.
    pub fn poll<T: for<'de> Deserialize<'de>>(
        &mut self,
        input: &mut impl Read,
    ) -> Result<Option<T>, FrameError> {
        if let Some(message) = self.take()? {
            return Ok(Some(message));
        }
        let mut chunk = [0u8; 16 * 1024];
        match input.read(&mut chunk) {
            Ok(0) => Err(FrameError::Io(std::io::ErrorKind::UnexpectedEof.into())),
            Ok(n) => {
                self.buf.extend_from_slice(&chunk[..n]);
                self.take()
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                Ok(None)
            }
            Err(e) => Err(e.into()),
        }
    }
    fn take<T: for<'de> Deserialize<'de>>(&mut self) -> Result<Option<T>, FrameError> {
        if self.buf.len() < 4 {
            return Ok(None);
        }
        let len = u32::from_be_bytes([self.buf[0], self.buf[1], self.buf[2], self.buf[3]]) as usize;
        if len > MAX_FRAME {
            return Err(FrameError::TooLarge(len));
        }
        if self.buf.len() < 4 + len {
            return Ok(None);
        }
        let message = serde_json::from_slice(&self.buf[4..4 + len])?;
        self.buf.drain(..4 + len);
        Ok(Some(message))
    }
}
pub fn read_frame<T: for<'de> Deserialize<'de>>(input: &mut impl Read) -> Result<T, FrameError> {
    let mut len = [0u8; 4];
    input.read_exact(&mut len)?;
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_FRAME {
        return Err(FrameError::TooLarge(len));
    }
    let mut body = vec![0u8; len];
    input.read_exact(&mut body)?;
    Ok(serde_json::from_slice(&body)?)
}
