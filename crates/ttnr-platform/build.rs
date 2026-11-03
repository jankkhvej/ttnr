//! Embeds `macos/Info.plist` into the examples (Now Playing spike) unless
//! `TTNR_NO_PLIST` is set, so both variants can be compared.

fn main() {
    println!("cargo:rerun-if-changed=macos/Info.plist");
    println!("cargo:rerun-if-env-changed=TTNR_NO_PLIST");
    let macos = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos");
    if macos && std::env::var_os("TTNR_NO_PLIST").is_none() {
        let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
        println!(
            "cargo:rustc-link-arg-examples=-Wl,-sectcreate,__TEXT,__info_plist,{dir}/macos/Info.plist"
        );
    }
}
