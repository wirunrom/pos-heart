//! Windows: embeds the app icon and file version in pos-heart.exe.
fn main() {
    println!("cargo:rerun-if-changed=../../packaging/icon/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../packaging/icon/icon.ico")
            .set("ProductName", "POS Heart")
            .set("FileDescription", "POS Heart")
            .set("CompanyName", "POS Heart");
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed the Windows icon: {e}");
        }
    }
}
