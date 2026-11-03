//! Embeds an Info.plist (bundle id `org.hcc07.ttnr`) into the `ttnr`
//! binary on macOS: Now Playing / media keys identify the app by it
//! (ADR-036).

fn main() {
    let plist = "crates/ttnr-platform/macos/Info.plist";
    println!("cargo:rerun-if-changed={plist}");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
        println!("cargo:rustc-link-arg-bins=-Wl,-sectcreate,__TEXT,__info_plist,{dir}/{plist}");
    }
}
