fn main() {
    let bin = std::env::var("CARGO_BIN_NAME").unwrap_or_default();
    let headless = std::env::var("CARGO_FEATURE_HEADLESS_SERVER").is_ok();
    let desktop = std::env::var("CARGO_FEATURE_DESKTOP").is_ok();
    // Skip Tauri codegen for headless-server lib/tests and the standalone API binary.
    if bin == "telegram-drive-server" || (headless && !desktop) {
        return;
    }
    tauri_build::build()
}
