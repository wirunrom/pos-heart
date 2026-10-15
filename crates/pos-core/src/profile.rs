//! The main register's shop profile (its `shop.json` and receipt logo),
//! mirrored into the database so paired registers and kitchen PCs print and
//! show the same shop. The desktop owns the files; this copy is only
//! published by the main register itself (a device-local worker command,
//! not a protocol request) and read by everyone through `Data`.
use crate::{Pos, Result, schema::app_settings};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

const KEY: &str = "shop_profile";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedProfile {
    /// The main register's `shop.json`, as written.
    pub shop: String,
    /// `receipt-logo.png`, when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<Vec<u8>>,
}
impl Pos {
    /// Replaces the published profile. Not authorized: only the main
    /// register's own worker calls it, mirroring its local files.
    pub fn publish_profile(&mut self, profile: &SharedProfile) -> Result<()> {
        diesel::replace_into(app_settings::table)
            .values((
                app_settings::key.eq(KEY),
                app_settings::value.eq(serde_json::to_string(profile)?),
            ))
            .execute(&mut self.conn)?;
        Ok(())
    }
    pub fn shared_profile(&mut self) -> Result<Option<SharedProfile>> {
        app_settings::table
            .find(KEY)
            .select(app_settings::value)
            .first::<String>(&mut self.conn)
            .optional()?
            .map(|json| Ok(serde_json::from_str(&json)?))
            .transpose()
    }
}
