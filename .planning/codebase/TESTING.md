# Testing Patterns

> Document version: 2026-05-28
> Scope: Telegram-Drive Rust backend (`app/src-tauri/src/`)

---

## 1. Test Frameworks

### Unit & Integration Tests: Built-in `#[test]`
- Standard Rust test framework with `#[cfg(test)]` modules.
- Async tests use `#[actix_rt::test]` for Actix-web integration tests.

### Test Dependencies
- `actix-web = "4"` — Web framework with test utilities
- `actix-rt = "2"` — Runtime for async tests
- No external test frameworks like `rstest`, `proptest`, or `mockall` are currently used.

---

## 2. Directory Structure

```text
app/src-tauri/
├── src/
│   ├── lib.rs                    # Tauri app setup
│   ├── api_routes.rs             # REST API + #[cfg(test)] module
│   ├── http_middleware.rs        # Middleware + #[cfg(test)] module
│   ├── route_registry.rs         # Route registry + OpenAPI contract test
│   ├── vpn_optimizer.rs          # Retry logic (no tests yet)
│   ├── db.rs                     # SQLite ops (no tests yet)
│   ├── ...
│   └── tests/                    # Integration tests directory
│       └── health_api.rs         # Health endpoint integration test
├── tests/                        # Additional integration tests (repo root)
│   └── integration/
│       ├── test-api.sh           # Bash smoke tests for Docker
│       └── test-api.ps1          # PowerShell smoke tests for Docker
└── Cargo.toml                    # Test configuration via features
```

### Key Rule
- **Unit tests** go inside `#[cfg(test)]` modules in the same file as the code under test.
- **Integration tests** go in `app/src-tauri/src/tests/` or `tests/integration/`.

---

## 3. Unit Test Patterns

### In-File Test Modules

From `app/src-tauri/src/api_routes.rs` (lines ~920-960):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_auth_valid_key() {
        let api_key = "test_key".to_string();
        let hashed = sha256::digest(&api_key);
        let result = check_auth(&api_key, &hashed);
        assert!(result);
    }

    #[test]
    fn test_check_auth_invalid_key() {
        let result = check_auth("wrong", "hash");
        assert!(!result);
    }
}
```

From `app/src-tauri/src/http_middleware.rs` (lines ~340-369):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_allows_first_request() {
        let limiter = RateLimit::new(10, Duration::from_secs(60));
        let client_ip = "127.0.0.1".to_string();
        assert!(limiter.check(&client_ip));
    }

    #[test]
    fn test_rate_limit_blocks_excessive_requests() {
        let limiter = RateLimit::new(1, Duration::from_secs(60));
        let client_ip = "127.0.0.1".to_string();
        assert!(limiter.check(&client_ip));
        assert!(!limiter.check(&client_ip));
    }
}
```

### Test Naming
- Use descriptive names explaining the scenario:
  - `test_check_auth_valid_key`
  - `test_rate_limit_allows_first_request`
  - `test_rate_limit_blocks_excessive_requests`

---

## 4. Integration Test Approach

### Actix-web Integration Tests

From `app/src-tauri/src/tests/health_api.rs`:
```rust
use actix_web::{test, web, App};

#[actix_rt::test]
async fn test_health_endpoint() {
    let app = test::init_service(
        App::new().route("/api/v1/health", web::get().to(health_handler))
    ).await;

    let req = test::TestRequest::get()
        .uri("/api/v1/health")
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "ok");
}
```

### Docker Smoke Tests

From `tests/integration/test-api.sh` (bash):
```bash
#!/bin/bash
set -e

BASE_URL="${API_BASE_URL:-http://localhost:1334}"

echo "Testing health endpoint..."
curl -sf "${BASE_URL}/api/v1/health" | grep -q '"status":"ok"'

echo "Testing config endpoint..."
curl -sf "${BASE_URL}/api/v1/config"

echo "Testing verify endpoint..."
curl -sf "${BASE_URL}/api/v1/verify" -H "X-API-Key: ${API_KEY}"

echo "All smoke tests passed!"
```

From `tests/integration/test-api.ps1` (PowerShell):
```powershell
$BaseUrl = $env:API_BASE_URL ?? "http://localhost:1334"

Write-Host "Testing health endpoint..."
$resp = Invoke-RestMethod -Uri "$BaseUrl/api/v1/health" -Method GET
if ($resp.status -ne "ok") { throw "Health check failed" }

Write-Host "All smoke tests passed!"
```

### OpenAPI Contract Test

From `app/src-tauri/src/route_registry.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_routes_documented_in_openapi() {
        let openapi = include_str!("../../docs/openapi.json");
        let spec: serde_json::Value = serde_json::from_str(openapi).unwrap();
        // Verify every route in ROUTES exists in the OpenAPI spec
        for route in ROUTES {
            let path = format!("/api/v1{}", route.path);
            assert!(
                spec["paths"].get(&path).is_some(),
                "Route {} {} not documented in OpenAPI",
                route.method,
                path
            );
        }
    }

    #[test]
    fn test_no_duplicate_routes() {
        let mut seen = std::collections::HashSet::new();
        for route in ROUTES {
            let key = format!("{} {}", route.method, route.path);
            assert!(seen.insert(key), "Duplicate route: {}", key);
        }
    }
}
```

---

## 5. Mocking Strategy

### Current Approach: Minimal Mocking
- The codebase does **not** use `mockall` or other mocking frameworks.
- Tests focus on:
  - Pure function logic (e.g., `check_auth`, `constant_time_eq`)
  - In-memory state behavior (e.g., rate limiter buckets)
  - HTTP endpoint structure (integration tests with Actix test server)

### Mock Mode in Production Code
- `app/src-tauri/src/commands/fs.rs` contains a mock mode when Telegram client is not connected:
  ```rust
  let client_opt = state.client.lock().await;
  if client_opt.is_none() {
      // Mock mode: return simulated success
      return Ok(json!({ "mock": true, "path": path }).to_string());
  }
  ```
- This is **not** test mocking but a runtime fallback for disconnected states.

### Recommended Future Mocking
- For database tests: use an in-memory SQLite connection.
- For Telegram client tests: introduce a trait-based abstraction around `grammers_client::Client`.

---

## 6. Coverage Expectations

### Current State
- **10 test functions** across the entire Rust backend (as of 2026-05-28).
- Tests exist in only 6 source files:
  1. `app/src-tauri/src/api_routes.rs` — 2 tests (`check_auth`)
  2. `app/src-tauri/src/http_middleware.rs` — 2 tests (rate limiter)
  3. `app/src-tauri/src/route_registry.rs` — 2 tests (OpenAPI contract, duplicates)
  4. `app/src-tauri/src/tests/health_api.rs` — 1 integration test
  5. `tests/integration/test-api.sh` — bash smoke tests
  6. `tests/integration/test-api.ps1` — PowerShell smoke tests

### Coverage Gaps
- No tests for `vpn_optimizer.rs` retry logic
- No tests for `db.rs` SQLite operations
- No tests for `commands/fs.rs` file operations
- No tests for `commands/auth.rs` Telegram client lifecycle
- No tests for `bandwidth.rs` throttling logic
- No tests for `sharing_core.rs` share link generation

### Target
- Aim for **80%+ line coverage** for new code.
- Use `cargo llvm-cov` for coverage reporting:
  ```bash
  cargo llvm-cov --html
  cargo llvm-cov --fail-under-lines 80
  ```

---

## 7. CI/CD Pipeline

### `.github/workflows/docker-api.yml`

```yaml
name: Docker API Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --no-default-features --features headless-server --lib --tests
      - run: docker build -t telegram-drive-api .
      - run: docker run -d -p 11334:1334 telegram-drive-api
      - run: sleep 5 && ./tests/integration/test-api.sh
```

### `.github/workflows/main.yml`
- Tauri app publishing for Windows, Linux, macOS.
- No test execution in the Tauri workflow (relies on Docker workflow for backend tests).

### Feature Flags in CI
- Tests run with `--no-default-features --features headless-server` to exclude Tauri desktop dependencies.
- This ensures the headless server binary compiles and tests pass without a GUI environment.

---

## 8. Test Configuration Files

### `app/src-tauri/Cargo.toml`
```toml
[features]
default = ["desktop"]
desktop = ["tauri"]
headless-server = ["actix-web", "actix-rt"]

[dev-dependencies]
actix-web = { version = "4", features = ["macros"] }
actix-rt = "2"
```

### `app/src-tauri/.cargo/config.toml`
```toml
[net]
git-fetch-with-cli = true
```
- Only network configuration; no test-specific settings.

### No `rust-toolchain.toml`
- Uses stable toolchain via GitHub Actions `dtolnay/rust-toolchain@stable`.

---

## 9. Running Tests

### All Tests
```bash
cd app/src-tauri
cargo test --no-default-features --features headless-server
```

### Unit Tests Only
```bash
cargo test --no-default-features --features headless-server --lib
```

### Integration Tests Only
```bash
cargo test --no-default-features --features headless-server --tests
```

### Specific Test
```bash
cargo test test_check_auth_valid_key --no-default-features --features headless-server
```

### With Output
```bash
cargo test -- --nocapture
```

---

## 10. Test Data & Fixtures

### Current Approach
- No formal fixture system.
- Tests use inline data:
  ```rust
  let api_key = "test_key".to_string();
  let hashed = sha256::digest(&api_key);
  ```

### Recommended Future Approach
- Create `tests/fixtures/` directory for:
  - Sample SQLite databases
  - Mock Telegram session files
  - Test configuration files

---

## 11. Testing Checklist

Before marking work complete:
- [ ] Unit tests added for new pure functions
- [ ] Integration tests added for new API endpoints
- [ ] Error paths tested (invalid input, missing auth, rate limits)
- [ ] Async code tested with `#[actix_rt::test]`
- [ ] Tests pass with `cargo test --no-default-features --features headless-server`
- [ ] No `unwrap()` or `expect()` in test assertions without justification
- [ ] Mock mode behavior verified if applicable

---

*End of TESTING.md*
