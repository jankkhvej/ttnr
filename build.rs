//! Embeds an Info.plist (bundle id `org.hcc07.ttnr`) into the `ttnr`
//! binary on macOS: Now Playing / media keys identify the app by it
//! (ADR-036). Its version is the crate's (`@VERSION@` in the template).

fn main() {
    let plist = "crates/ttnr-platform/macos/Info.plist";
    println!("cargo:rerun-if-changed={plist}");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let out = std::env::var("OUT_DIR").unwrap_or_default();
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let template = match std::fs::read_to_string(format!("{dir}/{plist}")) {
        Ok(t) => t,
        Err(e) => {
            println!("cargo:warning=cannot read {plist}: {e}");
            return;
        }
    };
    let path = format!("{out}/Info.plist");
    if let Err(e) = std::fs::write(&path, template.replace("@VERSION@", &version)) {
        println!("cargo:warning=cannot write {path}: {e}");
        return;
    }
    println!("cargo:rustc-link-arg-bins=-Wl,-sectcreate,__TEXT,__info_plist,{path}");
}
