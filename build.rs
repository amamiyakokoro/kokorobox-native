use std::env;

fn main() {
    println!("cargo:rerun-if-changed=native/macos_app_routing.mm");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    cc::Build::new()
        .cpp(true)
        .file("native/macos_app_routing.mm")
        .flag("-fobjc-arc")
        .flag("-fblocks")
        .flag("-std=c++17")
        .flag("-mmacosx-version-min=13.0")
        .compile("kokorobox_macos_app_routing");
    for framework in [
        "AppKit",
        "Foundation",
        "NetworkExtension",
        "SystemExtensions",
    ] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
}
