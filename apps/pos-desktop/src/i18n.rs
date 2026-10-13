use pos_core::Error;
use pos_domain::DomainError;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    Thai,
    English,
}
impl Language {
    pub fn text(self, thai: &'static str, english: &'static str) -> &'static str {
        match self {
            Self::Thai => thai,
            Self::English => english,
        }
    }
    /// Counted noun: Thai has no plural; English picks singular or plural.
    pub fn noun(
        self,
        n: u64,
        thai: &'static str,
        one: &'static str,
        many: &'static str,
    ) -> &'static str {
        match (self, n) {
            (Self::Thai, _) => thai,
            (Self::English, 1) => one,
            (Self::English, _) => many,
        }
    }
    pub fn payment_method(self, method: pos_domain::PaymentMethod) -> &'static str {
        match method {
            pos_domain::PaymentMethod::Cash => self.text("เงินสด", "Cash"),
            pos_domain::PaymentMethod::Transfer => self.text("โอน", "Transfer"),
            pos_domain::PaymentMethod::Card => self.text("บัตร", "Card"),
        }
    }
    /// How a sale was paid: one method, or "Split · cash + transfer".
    pub fn sale_method(self, sale: &pos_domain::Sale) -> String {
        if !sale.is_split() {
            return self.payment_method(sale.payment_method).to_owned();
        }
        let parts: Vec<&str> = sale
            .tenders
            .iter()
            .map(|t| self.payment_method(t.method))
            .collect();
        format!("{} · {}", self.text("แบ่งจ่าย", "Split"), parts.join(" + "))
    }
    /// Built-in roles in the current language; custom roles by their name.
    pub fn role_name(self, id: &str, stored: &str) -> String {
        match id {
            pos_domain::OWNER => self.text("เจ้าของร้าน", "Owner").into(),
            pos_domain::MANAGER => self.text("ผู้จัดการ", "Manager").into(),
            pos_domain::CASHIER => self.text("แคชเชียร์", "Cashier").into(),
            _ => stored.into(),
        }
    }
    pub fn permission(self, p: pos_domain::Permission) -> &'static str {
        use pos_domain::Permission as P;
        match p {
            P::Refund => self.text("คืนสินค้าบางรายการ", "Refund items"),
            P::Void => self.text("ยกเลิกทั้งบิล", "Void whole sale"),
            P::Discount => self.text("ส่วนลดเกินเพดาน", "Discount above the limit"),
            P::CashMovement => self.text("นำเงินเข้า-ออกลิ้นชัก", "Cash in / out"),
            P::CloseShift => self.text("นับเงินและปิดกะ", "Count & close shift"),
            P::Catalog => self.text("แก้ไขสินค้า", "Edit products"),
            P::Stock => self.text("ปรับสต็อก", "Adjust stock"),
            P::Reports => self.text("ดูรายงาน", "View reports"),
            P::Settings => self.text(
                "ตั้งค่าร้าน ราคา VAT เครื่องพิมพ์",
                "Shop, pricing, VAT & printer settings",
            ),
            P::ManageStaff => self.text("จัดการพนักงานและบทบาท", "Manage staff & roles"),
        }
    }
    pub fn access(self, a: pos_domain::Access) -> &'static str {
        match a {
            pos_domain::Access::Allow => self.text("ทำได้เอง", "Allowed"),
            pos_domain::Access::Approval => self.text("ต้องอนุมัติ", "Approval"),
            pos_domain::Access::Deny => self.text("ห้าม", "Denied"),
        }
    }
    pub fn error(self, error: &Error) -> String {
        let text = match error {
            Error::Domain(d) => match d {
                DomainError::InvalidMoney => self.text(
                    "กรุณาใส่จำนวนเงินไม่ติดลบ ทศนิยมไม่เกิน 2 ตำแหน่ง",
                    "Enter a nonnegative amount with at most 2 decimals.",
                ),
                DomainError::Overflow => self.text(
                    "จำนวนเงินหรือจำนวนสินค้าเกินขอบเขต",
                    "Amount or quantity exceeds the supported range.",
                ),
                DomainError::MissingProductFields => {
                    self.text("กรุณาระบุรหัสสินค้าและชื่อสินค้า", "Enter a product code and name.")
                }
                DomainError::TextTooLong => self.text(
                    "ข้อความยาวเกินกำหนด กรุณาย่อข้อความ",
                    "Text is too long. Shorten the name or note.",
                ),
                DomainError::EmptyCart => {
                    self.text("กรุณาเลือกสินค้าอย่างน้อยหนึ่งรายการ", "Add at least one product.")
                }
                DomainError::InvalidQuantity => self.text(
                    "จำนวนสินค้าต้องอยู่ระหว่าง 1 ถึง 999,999",
                    "Quantity must be between 1 and 999,999.",
                ),
                DomainError::InsufficientPayment => self.text(
                    "เงินที่รับน้อยกว่ายอดที่ต้องชำระ",
                    "Cash received is less than the total.",
                ),
                DomainError::MissingVoidReason => self.text(
                    "กรุณาระบุเหตุผลยกเลิกบิล",
                    "Enter a reason for voiding this sale.",
                ),
                DomainError::MissingDifferenceReason => self.text(
                    "ยอดนับไม่ตรง กรุณาระบุเหตุผลของส่วนต่างก่อนปิดกะ",
                    "The count is off. Enter a reason for the difference before closing.",
                ),
                DomainError::InvalidStockChange => self.text(
                    "จำนวนสต็อกไม่ถูกต้อง: รับเข้าต้องมากกว่า 0 นำออกต้องไม่เกินที่มีและต้องมีเหตุผล",
                    "Invalid stock change: receive more than 0; remove no more than in stock, with a reason.",
                ),
                DomainError::InvalidPromptPay => self.text(
                    "พร้อมเพย์ต้องเป็นเบอร์มือถือ 10 หลัก เลขประจำตัว 13 หลัก หรือ e-Wallet 15 หลัก",
                    "PromptPay must be a 10-digit mobile, 13-digit ID or 15-digit e-Wallet number.",
                ),
                DomainError::InvalidDiscount => self.text(
                    "ส่วนลดไม่ถูกต้อง: ใส่จำนวนเงินมากกว่า 0 หรือ 0.01–100%",
                    "Invalid discount: enter an amount above 0 or 0.01–100%.",
                ),
                DomainError::DiscountTooLarge => self.text(
                    "ส่วนลดมากกว่าราคา ลดได้ไม่เกินยอดของรายการหรือบิล",
                    "The discount is more than the price it applies to.",
                ),
                DomainError::InvalidStaff => self.text(
                    "ใส่ชื่อพนักงาน 1–60 ตัวอักษรและเลือกบทบาท",
                    "Enter a name of 1–60 characters and choose a role.",
                ),
                DomainError::InvalidPin => self.text(
                    "PIN ต้องเป็นตัวเลข 4–8 หลัก",
                    "The PIN must be 4–8 digits.",
                ),
                DomainError::InvalidTaxSettings => self.text(
                    "ข้อมูลภาษีไม่ครบ: ชื่อ เลขประจำตัวผู้เสียภาษี 13 หลัก (ตรวจเลขท้าย) สาขา 5 หลัก ที่อยู่ และอัตรา VAT",
                    "Tax details incomplete: name, valid 13-digit tax ID, 5-digit branch, address and VAT rate.",
                ),
                DomainError::InvalidBuyer => self.text(
                    "ข้อมูลผู้ซื้อไม่ครบ: ชื่อ เลขประจำตัวผู้เสียภาษี 13 หลักที่ถูกต้อง สาขา และที่อยู่",
                    "Buyer details incomplete: name, valid 13-digit tax ID, branch and address.",
                ),
                DomainError::InvalidPricing => self.text(
                    "ตั้งค่าราคาไม่ถูกต้อง: service charge 0–30% และปัดเศษ 0, 0.25, 0.50 หรือ 1 บาท",
                    "Invalid pricing: service charge 0–30% and rounding of 0, 0.25, 0.50 or 1 baht.",
                ),
                DomainError::InvalidRole => self.text(
                    "ชื่อบทบาท 1–40 ตัวอักษร เพดานส่วนลด 0–100% และจัดการพนักงานได้เฉพาะเจ้าของร้าน",
                    "Role names are 1–40 characters, limits 0–100%, and only owners manage staff.",
                ),
                DomainError::InvalidSplit => self.text(
                    "แบ่งจ่ายต้องใช้อย่างน้อย 2 วิธี และยอดรวมต้องเท่ายอดชำระพอดี",
                    "A split needs at least two methods that add up to the exact amount due.",
                ),
                DomainError::InvalidPromotion => self.text(
                    "โปรโมชันไม่ถูกต้อง: ตรวจชื่อ เงื่อนไข วัน เวลา วันที่ และคูปอง",
                    "Check the promotion's name, rule, days, hours, dates and coupon.",
                ),
                DomainError::InvalidBillSplit => self.text(
                    "เลือกรายการที่จะแยกอย่างน้อยหนึ่งรายการ และต้องเหลือในบิลเดิมอย่างน้อยหนึ่งรายการ",
                    "Choose at least one item to move and leave at least one on the bill.",
                ),
                DomainError::InvalidTable => self.text(
                    "ชื่อโต๊ะ 1–20 ตัวอักษร โซนไม่เกิน 40 ตัวอักษร ที่นั่ง 0–99",
                    "Table names are 1–20 characters, zones up to 40, seats 0–99.",
                ),
                DomainError::InvalidOptions => self.text(
                    "ตัวเลือกสินค้าไม่ครบหรือเปลี่ยนไปแล้ว กรุณาเลือกตัวเลือกใหม่",
                    "Options are missing or have changed. Choose the options again.",
                ),
                DomainError::CountMismatch => self.text(
                    "ยอดนับแยกตามชนิดเงินไม่ตรงกับยอดรวม กรุณานับใหม่",
                    "The denomination count does not match the total. Count again.",
                ),
            },
            Error::PaymentTotalChanged => self.text("ยอดสินค้าเปลี่ยนแล้ว ตรวจยอดใหม่และตรวจรับชำระอีกครั้ง", "The total changed. Review the refreshed amount and verify payment again."),
            Error::InvalidRefundMethod => self.text("ต้องคืนเงินตามช่องทางที่ชำระในบิลเดิม", "Refund using the original payment method."),
            Error::DuplicateCoupon => self.text(
                "มีโปรโมชันอื่นใช้รหัสคูปองนี้แล้ว",
                "Another promotion already uses this coupon code.",
            ),
            Error::RefundExceedsMethod => self.text(
                "ยอดคืนผ่านช่องทางนี้เกินที่ลูกค้าจ่ายด้วยช่องทางนี้ เลือกช่องทางอื่น",
                "That is more than this method paid. Choose another method.",
            ),
            Error::DuplicateSku => self.text(
                "รหัสสินค้านี้มีอยู่แล้ว กรุณาใช้รหัสอื่น",
                "This product code already exists. Choose another code.",
            ),
            Error::SaleVoided => self.text(
                "บิลนี้ยกเลิกทั้งบิลแล้ว คืนเพิ่มไม่ได้",
                "This sale was already voided. Nothing more can be refunded.",
            ),
            Error::PartiallyRefunded => self.text(
                "บิลนี้คืนสินค้าไปบางส่วนแล้ว ใช้คืนสินค้าที่เหลือแทนการยกเลิกทั้งบิล",
                "Some items were already refunded. Refund the remaining items instead of voiding.",
            ),
            Error::SignInRequired => self.text(
                "ใส่ PIN เพื่อเข้าใช้งานก่อน",
                "Sign in with your PIN first.",
            ),
            Error::ApprovalRequired(p) => {
                return format!(
                    "{}: {}",
                    self.text("ต้องให้ผู้จัดการอนุมัติ", "Needs a manager's approval"),
                    self.permission(*p)
                );
            }
            Error::WrongPin => self.text("PIN ไม่ถูกต้อง", "Wrong PIN."),
            Error::PinLocked(secs) => {
                return format!(
                    "{} {secs} {}",
                    self.text(
                        "ใส่ PIN ผิดหลายครั้ง ลองใหม่ในอีก",
                        "Too many wrong PINs. Try again in"
                    ),
                    self.text("วินาที", "s")
                );
            }
            Error::StaffNotFound => self.text(
                "ไม่พบพนักงาน หรือถูกปิดการใช้งานแล้ว",
                "Staff member not found or switched off.",
            ),
            Error::LastOwner => self.text(
                "ต้องมีเจ้าของร้านที่ใช้งานอยู่อย่างน้อย 1 คน (คนแรกต้องเป็นเจ้าของร้าน)",
                "Keep at least one active owner (the first staff member must be an owner).",
            ),
            Error::NotTaxReceipt => self.text(
                "บิลนี้ชำระก่อนเปิดใช้ VAT จึงออกใบกำกับภาษีไม่ได้",
                "This receipt was paid before VAT was set up, so it cannot get a tax invoice.",
            ),
            Error::SaleRefunded => self.text(
                "บิลนี้มีการคืนเงินแล้ว จึงออกใบกำกับภาษีเต็มรูปย้อนหลังไม่ได้",
                "This sale was refunded, so a full tax invoice can no longer be issued.",
            ),
            Error::Forbidden(p) => {
                return format!(
                    "{}: {}",
                    self.text("บทบาทนี้ไม่มีสิทธิ์", "This role may not"),
                    self.permission(*p)
                );
            }
            Error::RoleNotFound => self.text("ไม่พบบทบาทนี้", "Role not found."),
            Error::RoleInUse => self.text(
                "ยังมีพนักงานใช้บทบาทนี้ ย้ายพนักงานไปบทบาทอื่นก่อนลบ",
                "Staff still hold this role. Move them to another role first.",
            ),
            Error::TableNotFound => self.text(
                "ไม่พบโต๊ะนี้หรือปิดใช้งานแล้ว",
                "Table not found or switched off.",
            ),
            Error::TableBusy => self.text(
                "โต๊ะนี้มีบิลเปิดอยู่แล้ว เลือกโต๊ะอื่นหรือเปิดบิลเดิม",
                "This table already has an open bill. Pick another table or open that bill.",
            ),
            Error::DuplicateTable => self.text(
                "มีโต๊ะชื่อนี้ในโซนนี้แล้ว",
                "A table with this name already exists in the zone.",
            ),
            Error::TicketNotFound => self.text("ไม่พบใบครัวนี้", "Kitchen ticket not found."),
            Error::RefundExceedsSale => self.text(
                "จำนวนที่คืนเกินจำนวนที่ขายไป (หรือคืนไปแล้ว)",
                "That is more than was sold (or already refunded).",
            ),
            Error::StockChanged => self.text(
                "สต็อกเปลี่ยนระหว่างนับ (มีการขายหรือปรับยอด) กรุณานับใหม่",
                "Stock changed while you counted (a sale or adjustment). Count again.",
            ),
            Error::StockNotTracked => self.text(
                "สินค้านี้ไม่ได้นับสต็อก เปิดการนับสต็อกก่อน",
                "This product does not track stock. Turn tracking on first.",
            ),
            Error::DuplicateBarcode => self.text(
                "บาร์โค้ดนี้เป็นของสินค้าอื่นแล้ว",
                "This barcode already belongs to another product.",
            ),
            Error::NotForSale(name) => {
                return format!(
                    "{}: {name}",
                    self.text(
                        "สินค้าปิดขายอยู่ นำออกจากบิลหรือเปิดขายก่อน",
                        "Not for sale. Remove it from the bill or turn selling back on",
                    )
                );
            }
            Error::ProductUnavailable => self.text(
                "สินค้าเปลี่ยนแปลงหรือถูกปิดขาย กรุณาตรวจตะกร้า",
                "A product is unavailable. Review the cart.",
            ),
            Error::InsufficientStock(name) => {
                return format!(
                    "{}: {name}",
                    self.text("สินค้าไม่เพียงพอ", "Insufficient stock")
                );
            }
            Error::AlreadyPaid => self.text(
                "บิลนี้ชำระแล้ว กรุณาสร้างบิลใหม่",
                "This bill is already paid. Start a new bill.",
            ),
            Error::CartNotFound => self.text(
                "ไม่พบบิลที่พักไว้ กรุณารีเฟรช",
                "Held bill not found. Refresh and try again.",
            ),
            Error::UnsupportedSchema(_) => self.text(
                "ฐานข้อมูลมาจากแอปรุ่นใหม่กว่า กรุณาอัปเดตแอป",
                "This database needs a newer app version.",
            ),
            Error::CorruptData | Error::Data(_) => self.text(
                "ข้อมูลไม่ถูกต้อง กรุณาเก็บไฟล์ฐานข้อมูลไว้เพื่อตรวจสอบ",
                "Invalid stored data. Preserve the database for inspection.",
            ),
            Error::Database(_) | Error::Connection(_) => self.text(
                "บันทึกหรืออ่านข้อมูลไม่สำเร็จ กรุณาลองใหม่ รายการที่ล้มเหลวไม่เปลี่ยนบิลหรือสต็อก",
                "Database operation failed. Retry; failed operations leave sales and stock unchanged.",
            ),
            Error::Io(_) => self.text(
                "เข้าถึงไฟล์ไม่ได้ กรุณาตรวจโฟลเดอร์และสิทธิ์การเขียน",
                "Cannot access the file. Check the folder and write permissions.",
            ),
            Error::SaleNotFound => self.text(
                "ไม่พบบิลขาย กรุณารีเฟรช",
                "Sale not found. Refresh and try again.",
            ),
            Error::LegacyStockReviewRequired => self.text(
                "บิลเก่าไม่มีข้อมูลการตัดสต็อก กรุณาตรวจเลือกสินค้าที่ต้องคืนสต็อก",
                "This legacy receipt has no stock snapshot. Review which units to return.",
            ),
            Error::StockTrackingChanged(name) => {
                return format!(
                    "{}: {name}",
                    self.text(
                        "สินค้าที่เคยตัดสต็อกถูกปิดการติดตาม กรุณาเปิดติดตามสต็อกก่อนคืน",
                        "Enable stock tracking again before returning originally tracked units",
                    )
                );
            }
            Error::InvalidStockSelection => {
                self.text("รายการคืนสต็อกไม่ถูกต้อง", "Invalid legacy stock selection.")
            }
            Error::InvalidVoidTime => self.text(
                "เวลายกเลิกอยู่ก่อนเวลาขาย กรุณาตรวจนาฬิกาเครื่อง",
                "Cancellation precedes payment. Check the computer clock.",
            ),
            Error::NoOpenShift => self.text("กรุณาเปิดกะก่อนรับเงินหรือคืนเงิน", "Open a shift before taking or refunding payment."),
            Error::ShiftAlreadyOpen => self.text("มีกะเปิดอยู่แล้ว รีเฟรชแล้วตรวจสอบกะปัจจุบัน", "A shift is already open. Refresh and review it."),
            Error::ShiftChanged => self.text("กะหรือยอดเงินเปลี่ยนแล้ว กรุณาตรวจยอดใหม่ก่อนยืนยัน", "Shift or totals changed. Review the refreshed totals before confirming."),
            Error::ShiftClosed => self.text("กะนี้ปิดแล้ว กรุณารีเฟรช", "This shift is closed. Refresh to continue."),
            Error::InvalidCashMovement => self.text("ระบุยอดมากกว่า 0 และเหตุผลไม่เกิน 240 ตัวอักษร", "Enter an amount greater than zero and a reason up to 240 characters."),
            Error::ConflictingRequest => self.text("รายการนี้ถูกบันทึกด้วยข้อมูลอื่นแล้ว กรุณารีเฟรชก่อนทำต่อ", "This request already contains different data. Refresh before continuing."),
            Error::InvalidShiftTime => self.text("เวลาอยู่ก่อนรายการในกะ กรุณาตรวจนาฬิกาเครื่อง", "Shift time precedes recorded activity. Check the computer clock."),
            Error::InvalidReportDate => self.text(
                "วันที่อยู่นอกช่วงที่รองรับ",
                "Report date is outside the supported range.",
            ),
        };
        text.into()
    }
}
