//! Portable, checksummed recovery folders. Activation happens before opening SQLite.
use crate::shop::ShopSettings;
use pos_core::{BackupInfo, Pos};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

const DB: &str = "pos-heart.sqlite3";
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u32,
    pub created_at: i64,
    pub info: BackupInfo,
    pub files: Vec<Entry>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub name: String,
    pub size: u64,
    pub sha256: String,
}
#[derive(Clone)]
pub struct Review {
    pub source: PathBuf,
    pub manifest: Manifest,
    pub shop: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    version: u32,
    stage: String,
    previous: String,
}
fn invalid(message: &str) -> io::Error {
    io::Error::other(message)
}
fn business_file(name: &str) -> bool {
    matches!(
        name,
        DB | "shop.json" | "current-bill.txt" | crate::print::template::LOGO_FILE
    ) || name
        .strip_prefix("drawer-count-")
        .and_then(|s| s.strip_suffix(".json"))
        .is_some_and(|s| Uuid::parse_str(s).is_ok())
}
fn digest(path: &Path) -> io::Result<(u64, String)> {
    if !fs::symlink_metadata(path)?.file_type().is_file() {
        return Err(invalid("Backup files must be regular files"));
    }
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
        size = size
            .checked_add(n as u64)
            .ok_or_else(|| invalid("File too large"))?;
    }
    Ok((size, format!("{:x}", hash.finalize())))
}
fn sync_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}
fn sync_dir(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        fs::File::open(path)?.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}
fn copy_file(source: &Path, target: &Path) -> io::Result<()> {
    digest(source)?;
    let mut input = fs::File::open(source)?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)?;
    io::copy(&mut input, &mut output)?;
    output.sync_all()
}
fn validate_local(dir: &Path) -> io::Result<String> {
    let shop = ShopSettings::load(dir)?
        .map(|s| s.name)
        .unwrap_or_else(|| "POS Heart".into());
    let marker = dir.join("current-bill.txt");
    if marker.exists() {
        Uuid::parse_str(fs::read_to_string(marker)?.trim()).map_err(io::Error::other)?;
    }
    for file in fs::read_dir(dir)? {
        let file = file?;
        let name = file.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("drawer-count-") && business_file(&name) {
            let bytes = fs::read(file.path())?;
            if bytes.len() > 65536 {
                return Err(invalid("Drawer draft too large"));
            }
            let value: serde_json::Value = serde_json::from_slice(&bytes)?;
            let id = name
                .trim_start_matches("drawer-count-")
                .trim_end_matches(".json");
            if value.get("version").and_then(|v| v.as_u64()) != Some(1)
                || value.get("shift_id").and_then(|v| v.as_str()) != Some(id)
            {
                return Err(invalid("Unsupported drawer draft"));
            }
        }
    }
    Ok(shop)
}
pub fn create(pos: &mut Pos, data: &Path, destination: &Path) -> pos_core::Result<PathBuf> {
    pos.authorize_recovery()?;
    validate_local(data)?;
    fs::create_dir(destination)?;
    let result = (|| -> pos_core::Result<()> {
        pos.backup(&destination.join(DB))?;
        for file in fs::read_dir(data)? {
            let file = file?;
            let name = file.file_name();
            let name = name.to_string_lossy();
            if name != DB && business_file(&name) {
                copy_file(&file.path(), &destination.join(name.as_ref()))?;
            }
        }
        let info = pos_core::inspect_database(&destination.join(DB))?;
        let mut files = Vec::new();
        for file in fs::read_dir(destination)? {
            let file = file?;
            let name = file.file_name().to_string_lossy().into_owned();
            let (size, sha256) = digest(&file.path())?;
            files.push(Entry { name, size, sha256 });
        }
        files.sort_by(|a, b| a.name.cmp(&b.name));
        let manifest = Manifest {
            version: 1,
            created_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            info,
            files,
        };
        sync_write(
            &destination.join("manifest.json"),
            &serde_json::to_vec_pretty(&manifest)?,
        )?;
        sync_dir(destination)?;
        if let Some(parent) = destination.parent() {
            sync_dir(parent)?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(destination);
    }
    result?;
    Ok(destination.to_path_buf())
}
pub fn inspect(source: &Path) -> pos_core::Result<Review> {
    if !fs::symlink_metadata(source)?.file_type().is_dir() {
        return Err(invalid("Choose a backup folder").into());
    }
    let manifest_path = source.join("manifest.json");
    if fs::metadata(&manifest_path)?.len() > 65536 {
        return Err(invalid("Backup manifest too large").into());
    }
    let manifest: Manifest = serde_json::from_slice(&fs::read(manifest_path)?)?;
    if manifest.version != 1 || manifest.files.is_empty() || manifest.files.len() > 128 {
        return Err(invalid("Unsupported backup format").into());
    }
    let mut names = std::collections::HashSet::new();
    for entry in &manifest.files {
        if !business_file(&entry.name)
            || !names.insert(&entry.name)
            || digest(&source.join(&entry.name))? != (entry.size, entry.sha256.clone())
        {
            return Err(invalid("Backup checksum or filename is invalid").into());
        }
    }
    if !names.contains(&DB.to_owned()) {
        return Err(invalid("Backup has no database").into());
    }
    // Only listed files may influence recovery; reject unlisted business files.
    for file in fs::read_dir(source)? {
        let name = file?.file_name().to_string_lossy().into_owned();
        if name != "manifest.json" && !names.contains(&name) {
            return Err(invalid("Unlisted backup file").into());
        }
    }
    if pos_core::inspect_database(&source.join(DB))? != manifest.info {
        return Err(invalid("Backup database does not match manifest").into());
    }
    let shop = validate_local(source)?;
    Ok(Review {
        source: source.to_path_buf(),
        manifest,
        shop,
    })
}
fn location(data: &Path) -> io::Result<(PathBuf, String, PathBuf)> {
    let absolute = if data.is_absolute() {
        data.to_path_buf()
    } else {
        std::env::current_dir()?.join(data)
    };
    let parent = absolute
        .parent()
        .ok_or_else(|| invalid("Data folder needs a parent"))?
        .to_path_buf();
    let name = absolute
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| invalid("Invalid data folder"))?
        .to_owned();
    Ok((
        parent.clone(),
        name.clone(),
        parent.join(format!(".{name}-restore.json")),
    ))
}
/// Pin the reviewed backup again; no live business file changes before restart.
/// The worker stops after this succeeds so there can be no later writes.
pub fn prepare(pos: &mut Pos, data: &Path, reviewed: &Review) -> pos_core::Result<PathBuf> {
    pos.authorize_recovery()?;
    let fresh = inspect(&reviewed.source)?;
    if fresh.manifest != reviewed.manifest {
        return Err(invalid("Backup changed; inspect it again").into());
    }
    let (parent, name, journal) = location(data)?;
    if journal.exists() {
        return Err(invalid("A restore is already pending").into());
    }
    let token = Uuid::new_v4();
    let pending = Pending {
        version: 1,
        stage: format!(".{name}-restore-{token}"),
        previous: format!(".{name}-before-{token}"),
    };
    let stage = parent.join(&pending.stage);
    let safety_name = format!("before-recovery-{token}.posbackup");
    fs::create_dir(&stage)?;
    let result = (|| -> pos_core::Result<()> {
        for entry in &fresh.manifest.files {
            copy_file(&fresh.source.join(&entry.name), &stage.join(&entry.name))?;
        }
        // Verify the copied bytes, including a source changed while it was being copied.
        for entry in &fresh.manifest.files {
            if digest(&stage.join(&entry.name))? != (entry.size, entry.sha256.clone()) {
                return Err(invalid("Backup changed during recovery").into());
            }
        }
        validate_local(&stage)?;
        // Migrate a disposable copy and exercise typed readers before scheduling activation.
        {
            let mut candidate = Pos::open(stage.join(DB))?;
            candidate.validate_recovery_data()?;
            candidate.catalog_counts()?;
            candidate.history(0)?;
            candidate.held()?;
            candidate.roles()?;
            candidate.tax_settings()?;
            candidate.pricing_settings()?;
            candidate.active_shift()?;
        }
        for name in [
            "printer.json",
            "theme.txt",
            "density.txt",
            "language.txt",
            "window.txt",
        ] {
            if data.join(name).exists() {
                copy_file(&data.join(name), &stage.join(name))?;
            }
        }
        create(pos, data, &data.join(&safety_name))?;
        sync_dir(&stage)?;
        let temporary = parent.join(format!(".{name}-journal-{token}.tmp"));
        sync_write(&temporary, &serde_json::to_vec(&pending)?)?;
        fs::rename(&temporary, &journal)?;
        sync_dir(&parent)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&stage);
    }
    result?;
    Ok(parent.join(pending.previous).join(safety_name))
}
/// Recover an interrupted directory swap. Called before settings or SQLite are read.
pub fn activate_pending(data: &Path) -> io::Result<()> {
    let (parent, name, journal) = location(data)?;
    let bytes = match fs::read(&journal) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let pending: Pending = serde_json::from_slice(&bytes)?;
    if pending.version != 1
        || !pending.stage.starts_with(&format!(".{name}-restore-"))
        || !pending.previous.starts_with(&format!(".{name}-before-"))
        || Path::new(&pending.stage).components().count() != 1
        || Path::new(&pending.previous).components().count() != 1
    {
        return Err(invalid("Invalid recovery journal"));
    }
    let stage = parent.join(pending.stage);
    let old = parent.join(pending.previous);
    if stage.exists() {
        // Validate again before moving the original; a damaged stage leaves it intact.
        pos_core::inspect_database(&stage.join(DB)).map_err(io::Error::other)?;
        validate_local(&stage)?;
        if data.exists() {
            if old.exists() {
                return Err(invalid("Recovery destination already exists"));
            }
            fs::rename(data, &old)?;
            sync_dir(&parent)?;
        } else if !old.exists() {
            return Err(invalid("Original recovery folder is missing"));
        }
        if let Err(e) = fs::rename(&stage, data) {
            if !data.exists() {
                let _ = fs::rename(&old, data);
            }
            return Err(e);
        }
    } else if !data.exists() || !old.exists() {
        return Err(invalid(
            "Recovery stage is missing; original data was preserved",
        ));
    }
    sync_dir(&parent)?;
    fs::remove_file(journal)?;
    sync_dir(&parent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pos_domain::{Cart, CartLine, Money, ProductInput};
    fn populate(data: &Path) -> Result<Pos, Box<dyn std::error::Error>> {
        fs::create_dir(data)?;
        let mut pos = Pos::open(data.join(DB))?;
        ShopSettings {
            name: "ร้านทดสอบ".into(),
            setup_complete: true,
            ..Default::default()
        }
        .save(data)?;
        let product = pos.save_product(ProductInput {
            sku: "P1".into(),
            name: "น้ำ".into(),
            price: "12.75".parse()?,
            stock: Some(10),
            ..Default::default()
        })?;
        pos.open_shift(Uuid::new_v4(), Money::ZERO, 1)?;
        let mut cart = Cart::new();
        cart.lines.push(CartLine {
            product_id: product.id,
            quantity: 1,
            note: String::new(),
            options: vec![],
            discount: None,
        });
        pos.checkout(&cart, "20".parse()?, 2)?;
        let mut held = Cart::new();
        held.lines.push(CartLine {
            product_id: product.id,
            quantity: 2,
            note: String::new(),
            options: vec![],
            discount: None,
        });
        pos.hold(&held, 3)?;
        fs::write(data.join("current-bill.txt"), held.id.to_string())?;
        fs::write(data.join("printer.json"), "device-local")?;
        fs::write(data.join("language.txt"), "en")?;
        Ok(pos)
    }
    #[test]
    fn complete_backup_restore_preserves_previous_and_device_settings()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let data = temp.path().join("shop");
        let source = temp.path().join("backup");
        let mut pos = populate(&data)?;
        create(&mut pos, &data, &source)?;
        assert!(!source.join("printer.json").exists());
        assert!(!source.join("language.txt").exists());
        let reviewed = inspect(&source)?;
        assert_eq!(reviewed.manifest.info.sales, 1);
        assert_eq!(reviewed.shop, "ร้านทดสอบ");
        pos.save_product(ProductInput {
            sku: "NEW".into(),
            name: "New".into(),
            price: Money::ZERO,
            ..Default::default()
        })?;
        ShopSettings {
            name: "Changed".into(),
            setup_complete: true,
            ..Default::default()
        }
        .save(&data)?;
        let original = fs::read(data.join("shop.json"))?;
        let previous = prepare(&mut pos, &data, &reviewed)?;
        assert_eq!(fs::read(data.join("shop.json"))?, original);
        drop(pos);
        activate_pending(&data)?;
        activate_pending(&data)?;
        assert_eq!(
            ShopSettings::load(&data)?.ok_or("shop missing")?.name,
            "ร้านทดสอบ"
        );
        assert_eq!(
            fs::read_to_string(data.join("printer.json"))?,
            "device-local"
        );
        assert_eq!(fs::read_to_string(data.join("language.txt"))?, "en");
        let mut restored = Pos::open(data.join(DB))?;
        assert_eq!(restored.catalog_counts()?.total, 1);
        assert_eq!(restored.history(0)?.len(), 1);
        let id = Uuid::parse_str(fs::read_to_string(data.join("current-bill.txt"))?.trim())?;
        assert_eq!(restored.resume(id)?.0.lines[0].quantity, 2);
        let mut prior = Pos::open(previous.join(DB))?;
        assert_eq!(prior.catalog_counts()?.total, 2);
        assert_eq!(fs::read(previous.join("shop.json"))?, original);
        Ok(())
    }
    #[test]
    fn corruption_future_formats_and_changed_reviews_refuse_without_touching_live_data()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let data = temp.path().join("shop");
        let source = temp.path().join("backup");
        let mut pos = populate(&data)?;
        create(&mut pos, &data, &source)?;
        let review = inspect(&source)?;
        fs::write(source.join("shop.json"), "broken")?;
        assert!(inspect(&source).is_err());
        assert!(prepare(&mut pos, &data, &review).is_err());
        assert_eq!(pos.catalog_counts()?.total, 1);
        let source2 = temp.path().join("backup2");
        create(&mut pos, &data, &source2)?;
        let mut manifest: Manifest =
            serde_json::from_slice(&fs::read(source2.join("manifest.json"))?)?;
        manifest.version = 2;
        fs::write(
            source2.join("manifest.json"),
            serde_json::to_vec(&manifest)?,
        )?;
        assert!(inspect(&source2).is_err());
        manifest.version = 1;
        manifest.files[0].name = "../outside".into();
        fs::write(
            source2.join("manifest.json"),
            serde_json::to_vec(&manifest)?,
        )?;
        assert!(inspect(&source2).is_err());
        assert!(!location(&data)?.2.exists());
        Ok(())
    }
    #[test]
    fn restart_completes_interrupted_swap_and_invalid_stage_keeps_original()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let data = temp.path().join("shop");
        let source = temp.path().join("backup");
        let mut pos = populate(&data)?;
        create(&mut pos, &data, &source)?;
        let review = inspect(&source)?;
        prepare(&mut pos, &data, &review)?;
        drop(pos);
        let (parent, _, journal) = location(&data)?;
        let pending: Pending = serde_json::from_slice(&fs::read(&journal)?)?;
        fs::rename(&data, parent.join(&pending.previous))?;
        activate_pending(&data)?;
        assert!(data.join(DB).exists());
        assert!(!journal.exists());
        let mut pos = Pos::open(data.join(DB))?;
        prepare(&mut pos, &data, &review)?;
        drop(pos);
        let pending: Pending = serde_json::from_slice(&fs::read(&journal)?)?;
        fs::write(parent.join(pending.stage).join(DB), "broken")?;
        assert!(activate_pending(&data).is_err());
        assert!(data.join(DB).exists());
        Ok(())
    }
}
