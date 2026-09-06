fn main() {
    for key in [
        "AB_BUILD_TIME",
        "AB_BUILD_USER",
        "AB_BUILD_GIT_HASH",
        "AB_BUILD_GIT_BRANCH",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
        if let Ok(value) = std::env::var(key) {
            if !value.trim().is_empty() {
                println!("cargo:rustc-env={key}={value}");
            }
        }
    }
}
