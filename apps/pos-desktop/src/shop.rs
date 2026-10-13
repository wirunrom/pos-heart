//! Local shop preferences. Kept separate from immutable sales and database backups.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::Path,
};
use uuid::Uuid;

/// Former descriptive shop type. No longer shown or edited, but kept in
/// version-1 `shop.json` so existing profiles still load and older app
/// versions can still read profiles written by this one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShopKind {
    Retail,
    Food,
    #[default]
    Mixed,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShopSettings {
    pub version: u32,
    pub name: String,
    pub kind: ShopKind,
    pub start_in_sell: bool,
    pub setup_complete: bool,
    /// PromptPay ID (digits) for transfer QR codes; empty when not set.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub promptpay: String,
    /// Layout of printed receipts and kitchen tickets.
    #[serde(
        default,
        skip_serializing_if = "crate::print::template::Templates::is_default"
    )]
    pub templates: crate::print::template::Templates,
}
impl Default for ShopSettings {
    fn default() -> Self {
        Self {
            version: 1,
            name: "POS Heart".into(),
            kind: ShopKind::Mixed,
            start_in_sell: false,
            setup_complete: false,
            promptpay: String::new(),
            templates: Default::default(),
        }
    }
}
impl ShopSettings {
    pub fn load(dir: &Path) -> io::Result<Option<Self>> {
        let bytes = match fs::read(dir.join("shop.json")) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let settings: Self = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
        settings.validate()?;
        Ok(Some(settings))
    }
    fn validate(&self) -> io::Result<()> {
        if self.version != 1 {
            return Err(io::Error::other("Unsupported shop settings version"));
        }
        if self.name.trim().is_empty() || self.name.chars().count() > 80 {
            return Err(io::Error::other("Shop name must contain 1–80 characters"));
        }
        self.templates.validate().map_err(io::Error::other)?;
        if !self.promptpay.is_empty() {
            pos_domain::PromptPayId::parse(&self.promptpay).map_err(io::Error::other)?;
        }
        Ok(())
    }
    /// The configured PromptPay ID, if any and valid.
    pub fn promptpay_id(&self) -> Option<pos_domain::PromptPayId> {
        pos_domain::PromptPayId::parse(&self.promptpay).ok()
    }
    pub fn save(&self, dir: &Path) -> io::Result<()> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        let temp = dir.join(format!(".shop-{}.tmp", Uuid::new_v4()));
        let result = (|| {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&temp, dir.join("shop.json"))
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferences_round_trip_and_invalid_update_preserves_saved_profile() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        assert_eq!(ShopSettings::load(dir.path())?, None);
        let mut shop = ShopSettings {
            name: "ร้านหัวใจ / Heart".into(),
            setup_complete: true,
            start_in_sell: true,
            ..Default::default()
        };
        shop.save(dir.path())?;
        assert_eq!(ShopSettings::load(dir.path())?, Some(shop.clone()));
        let old = fs::read(dir.path().join("shop.json"))?;
        shop.name = "  ".into();
        assert!(shop.save(dir.path()).is_err());
        assert_eq!(fs::read(dir.path().join("shop.json"))?, old);
        Ok(())
    }
    #[test]
    fn malformed_and_future_profiles_are_refused_without_rewrite() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let future = ShopSettings {
            version: 99,
            ..Default::default()
        };
        let bytes = serde_json::to_vec(&future).map_err(io::Error::other)?;
        fs::write(dir.path().join("shop.json"), &bytes)?;
        assert!(ShopSettings::load(dir.path()).is_err());
        assert_eq!(fs::read(dir.path().join("shop.json"))?, bytes);
        fs::write(dir.path().join("shop.json"), "{broken")?;
        assert!(ShopSettings::load(dir.path()).is_err());
        Ok(())
    }
    #[test]
    fn existing_shop_type_still_loads_and_is_kept_on_save() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        fs::write(
            dir.path().join("shop.json"),
            r#"{"version":1,"name":"ร้านอาหาร","kind":"food","start_in_sell":true,"setup_complete":true}"#,
        )?;
        let mut shop =
            ShopSettings::load(dir.path())?.ok_or_else(|| io::Error::other("missing"))?;
        assert_eq!(shop.kind, ShopKind::Food);
        shop.name = "ร้านใหม่".into();
        shop.save(dir.path())?;
        let text = fs::read_to_string(dir.path().join("shop.json"))?;
        assert!(text.contains("\"kind\": \"food\""), "{text}");
        assert!(!text.contains("promptpay"), "{text}");
        Ok(())
    }
    #[test]
    fn promptpay_is_validated_and_optional() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let mut shop = ShopSettings {
            promptpay: "0812345678".into(),
            ..Default::default()
        };
        shop.save(dir.path())?;
        let loaded = ShopSettings::load(dir.path())?.ok_or_else(|| io::Error::other("missing"))?;
        assert_eq!(
            loaded.promptpay_id().map(|p| p.masked()),
            Some("08x-xxx-5678".into())
        );
        shop.promptpay = "12345".into();
        assert!(shop.save(dir.path()).is_err());
        Ok(())
    }
}
