use rustc_version::{version_meta, Channel};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // Set cfg flags depending on release channel
    let channel = match version_meta().expect("version_meta").channel {
        Channel::Stable => "CHANNEL_STABLE",
        Channel::Beta => "CHANNEL_BETA",
        Channel::Nightly => "CHANNEL_NIGHTLY",
        Channel::Dev => "CHANNEL_DEV",
    };
    println!("cargo:rustc-cfg={channel}");
}
