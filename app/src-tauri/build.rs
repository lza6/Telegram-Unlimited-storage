#[cfg(feature = "desktop")]
fn main() {
    tauri_build::build()
}

#[cfg(not(feature = "desktop"))]
fn main() {}
