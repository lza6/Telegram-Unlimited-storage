//! Canonical HTTP route lists for OpenAPI contract tests.
//! Headless mounts all routes on one port; desktop splits REST API vs stream server.

/// Headless unified gateway (default 1334): API + Web + legacy upload + `/d/*` + metrics.
pub const HEADLESS_ROUTES: &[(&str, &str)] = &[
    ("GET", "/api/v1/settings"),
    ("PUT", "/api/v1/settings"),
    ("GET", "/api/v1/network"),
    ("PUT", "/api/v1/network"),
    ("GET", "/api/v1/health"),
    ("GET", "/health/live"),
    ("GET", "/health/ready"),
    ("GET", "/api/v1/auth/status"),
    ("GET", "/api/v1/transport"),
    ("POST", "/api/v1/transport/mode"),
    ("POST", "/api/v1/auth/phone/request"),
    ("POST", "/api/v1/auth/phone/sign-in"),
    ("POST", "/api/v1/auth/phone/password"),
    ("POST", "/api/v1/auth/qr/start"),
    ("GET", "/api/v1/auth/qr/poll"),
    ("GET", "/api/v1/folders"),
    ("GET", "/api/v1/files"),
    ("POST", "/api/v1/files"),
    ("GET", "/api/v1/files/{message_id}"),
    ("GET", "/api/v1/files/{message_id}/download"),
    ("POST", "/api/v1/files/bulk"),
    ("POST", "/api/v1/files/rebuild-index"),
    ("GET", "/api/v1/files/search"),
    ("GET", "/api/v1/shares"),
    ("POST", "/api/v1/shares"),
    ("DELETE", "/api/v1/shares/{id}"),
    ("POST", "/verify"),
    ("GET", "/config"),
    ("POST", "/upload"),
    ("POST", "/upload_chunk"),
    ("POST", "/upload_progress_token"),
    ("GET", "/upload_status"),
    ("GET", "/upload_events"),
    ("GET", "/upload_ws"),
    ("POST", "/merge_chunks"),
    ("GET", "/d"),
    ("GET", "/d/signed"),
    ("GET", "/metrics"),
    ("GET", "/d/{token}"),
    ("POST", "/d/{token}/verify"),
    ("GET", "/stream/{folder_id}/{message_id}"),
];

/// Desktop REST API (default 8550): subset only. Stream `/d/*` + `/stream/*` on port 14201.
pub const DESKTOP_API_ROUTES: &[(&str, &str)] = &[
    ("GET", "/api/v1/settings"),
    ("PUT", "/api/v1/settings"),
    ("GET", "/api/v1/network"),
    ("PUT", "/api/v1/network"),
    ("GET", "/api/v1/health"),
    ("GET", "/health/live"),
    ("GET", "/health/ready"),
    ("GET", "/api/v1/auth/status"),
    ("GET", "/api/v1/transport"),
    ("POST", "/api/v1/transport/mode"),
    ("POST", "/api/v1/auth/phone/request"),
    ("POST", "/api/v1/auth/phone/sign-in"),
    ("POST", "/api/v1/auth/phone/password"),
    ("POST", "/api/v1/auth/qr/start"),
    ("GET", "/api/v1/auth/qr/poll"),
    ("GET", "/api/v1/folders"),
    ("GET", "/api/v1/files"),
    ("POST", "/api/v1/files"),
    ("GET", "/api/v1/files/{message_id}"),
    ("GET", "/api/v1/files/{message_id}/download"),
    ("POST", "/api/v1/files/bulk"),
    ("POST", "/api/v1/files/rebuild-index"),
    ("GET", "/api/v1/files/search"),
    ("GET", "/api/v1/shares"),
    ("POST", "/api/v1/shares"),
    ("DELETE", "/api/v1/shares/{id}"),
];

/// Alias for OpenAPI parity tests (Headless full surface).
pub const IMPLEMENTED_ROUTES: &[(&str, &str)] = HEADLESS_ROUTES;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeSet, HashMap};

    fn load_openapi_routes() -> BTreeSet<(String, String)> {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let path = std::path::Path::new(manifest)
            .join("..")
            .join("..")
            .join("docs")
            .join("openapi.json");
        let text = std::fs::read_to_string(&path).expect("openapi.json readable");
        let doc: serde_json::Value = serde_json::from_str(&text).expect("valid openapi json");
        let mut set = BTreeSet::new();
        let paths = doc
            .get("paths")
            .and_then(|p| p.as_object())
            .expect("paths object");
        for (path, methods) in paths {
            let obj = methods.as_object().expect("path methods");
            for method in obj.keys() {
                if method.starts_with('x') || method == "parameters" {
                    continue;
                }
                set.insert((method.to_uppercase(), path.clone()));
            }
        }
        set
    }

    fn route_set(routes: &[(&str, &str)]) -> BTreeSet<(String, String)> {
        routes
            .iter()
            .map(|(m, p)| (m.to_string(), p.to_string()))
            .collect()
    }

    #[test]
    fn openapi_matches_implementation_exactly() {
        let openapi = load_openapi_routes();
        let implemented = route_set(HEADLESS_ROUTES);
        assert_eq!(
            openapi, implemented,
            "OpenAPI paths must match route_registry::HEADLESS_ROUTES"
        );
    }

    #[test]
    fn desktop_api_routes_are_headless_subset() {
        let headless = route_set(HEADLESS_ROUTES);
        for route in DESKTOP_API_ROUTES {
            let key = (route.0.to_string(), route.1.to_string());
            assert!(
                headless.contains(&key),
                "desktop route not in headless registry: {} {}",
                route.0,
                route.1
            );
        }
    }

    #[test]
    fn no_duplicate_routes() {
        let mut seen = HashMap::new();
        for (method, path) in HEADLESS_ROUTES {
            let key = format!("{method} {path}");
            assert!(
                seen.insert(key.clone(), ()).is_none(),
                "duplicate route: {key}"
            );
        }
    }
}
