fn main() {
    println!("cargo:rustc-check-cfg=cfg(nightly)");
    println!("cargo:rustc-check-cfg=cfg(target_pointer_width, values(\"8\", \"16\", \"32\", \"64\"))");
    if let Some(true) = version_check::is_feature_flaggable() {
        println!("cargo:rustc-cfg=nightly");
    }
}
