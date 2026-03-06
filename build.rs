use std::env;

fn main() {
    embuild::espidf::sysenv::output();

    // Set OTA key from environment or use default
    let ota_key = env::var("OTA_KEY").unwrap_or_else(|_| "change-me-in-production".to_string());
    println!("cargo:rustc-env=OTA_KEY={}", ota_key);

    // Track when to rebuild
    println!("cargo:rerun-if-changed=static");
    println!("cargo:rerun-if-changed=partitions.csv");
    println!("cargo:rerun-if-changed=sdkconfig.defaults");
}
