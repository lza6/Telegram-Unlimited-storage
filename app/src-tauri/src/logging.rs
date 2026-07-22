use std::io::Write;

/// Initialize env_logger; when `LOG_FORMAT=json`, emit one JSON object per line.
pub fn init_from_env() {
    let json = std::env::var("LOG_FORMAT")
        .map(|v| v.eq_ignore_ascii_case("json"))
        .unwrap_or(false);

    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));

    if json {
        builder.format(|buf, record| {
            let entry = serde_json::json!({
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "level": record.level().to_string(),
                "target": record.target(),
                "message": format!("{}", record.args()),
            });
            writeln!(buf, "{}", entry)
        });
    }

    builder.init();
}
