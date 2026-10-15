//! mDNS needs a real network interface and multicast, which CI runners may
//! not allow: run with `cargo test -p pos-hub --test discovery -- --ignored`.
use pos_hub::discovery::{Advertisement, browse};
use std::time::Duration;

#[test]
#[ignore = "needs multicast on a real network"]
fn an_announced_hub_is_found_with_its_fingerprint() -> Result<(), Box<dyn std::error::Error>> {
    let fingerprint = [7u8; 32];
    let _ad = Advertisement::start("ร้านทดสอบ", 47_899, &fingerprint)?;
    let hex: String = fingerprint.iter().map(|b| format!("{b:02x}")).collect();
    let found = browse(Duration::from_secs(4))?;
    let hub = found
        .iter()
        .find(|f| f.fingerprint == hex)
        .ok_or("hub not found")?;
    assert_eq!(hub.address.port(), 47_899);
    assert_eq!(hub.name, "ร้านทดสอบ");
    Ok(())
}
