fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(target_os = "linux")]
    println!("cargo:rustc-link-lib=dylib=stdc++");

    if let Ok(target) = std::env::var("TARGET") {
        println!("cargo:rustc-env=COMPILED_TARGET_TRIPLE={target}");
    }
}
