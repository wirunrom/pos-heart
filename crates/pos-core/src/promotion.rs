//! Promotions: stored as JSON with a use counter. Checkout reads them inside
//! its transaction and counts each promotion once per bill it applies to.
use crate::{Error, Pos, Result, schema::promotions};
use diesel::prelude::*;
use pos_domain::{Permission, Promotion};
use uuid::Uuid;

pub(crate) fn load(conn: &mut SqliteConnection) -> Result<Vec<Promotion>> {
    promotions::table
        .order(promotions::updated_at.desc())
        .select((promotions::promo_json, promotions::uses))
        .load::<(String, i64)>(conn)?
        .into_iter()
        .map(|(json, uses)| {
            let mut p: Promotion = serde_json::from_str(&json)?;
            p.uses = u32::try_from(uses).map_err(|_| Error::CorruptData)?;
            Ok(p)
        })
        .collect()
}
/// One more bill used each of `ids`.
pub(crate) fn count_uses(conn: &mut SqliteConnection, ids: &[Uuid]) -> Result<()> {
    for id in ids {
        diesel::update(promotions::table.find(id.to_string()))
            .set(promotions::uses.eq(promotions::uses + 1))
            .execute(conn)?;
    }
    Ok(())
}
impl Pos {
    pub fn promotions(&mut self) -> Result<Vec<Promotion>> {
        load(&mut self.conn)
    }
    /// Adds or edits a promotion (Settings permission). Its use count stays.
    pub fn save_promotion(&mut self, promo: &Promotion, now: i64) -> Result<()> {
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            crate::staff::authorize(conn, &session, Some(Permission::Settings))?;
            let mut promo = promo.clone();
            promo.name = promo.name.trim().to_owned();
            promo.coupon = promo.coupon.trim().to_uppercase();
            promo.validate()?;
            if promo.is_coupon()
                && load(conn)?
                    .iter()
                    .any(|p| p.id != promo.id && pos_domain::same_code(&p.coupon, &promo.coupon))
            {
                return Err(Error::DuplicateCoupon);
            }
            let json = serde_json::to_string(&promo)?;
            diesel::insert_into(promotions::table)
                .values((
                    promotions::id.eq(promo.id.to_string()),
                    promotions::promo_json.eq(&json),
                    promotions::updated_at.eq(now),
                ))
                .on_conflict(promotions::id)
                .do_update()
                .set((
                    promotions::promo_json.eq(&json),
                    promotions::updated_at.eq(now),
                ))
                .execute(conn)?;
            Ok(())
        })
    }
    /// Receipts keep the promotion's name; deleting only stops future use.
    pub fn delete_promotion(&mut self, id: Uuid) -> Result<()> {
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            crate::staff::authorize(conn, &session, Some(Permission::Settings))?;
            diesel::delete(promotions::table.find(id.to_string())).execute(conn)?;
            Ok(())
        })
    }
}
