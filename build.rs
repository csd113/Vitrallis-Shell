fn main() {
    if let Ok(target) = std::env::var("TARGET") {
        println!("cargo:rustc-env=VITRALLIS_BUILD_TARGET={target}");
    }
}
