# Phase Execution Tracker — v3→v4 Upgrade

## Phase 1: Security Hardening ✅ DONE

| ID | Issue | Status | Files Modified |
|----|-------|--------|----------------|
| H-01 | Argon2id upgrade hint | ✅ | password_kdf.rs, commands/api_settings.rs, share_routes.rs, presigned_url.rs |
| H-02 | Timing attack prevention | ✅ | admin_routes.rs, webdav_routes.rs |
| H-03 | Filename sanitization | ✅ | http_download.rs |
| H-04 | CSP hardening | ✅ | http_middleware.rs |

## Phase 2: Backend Architecture ✅ DONE (5/6)

| ID | Issue | Status | Files Modified |
|----|-------|--------|----------------|
| M-01 | SQLite connection pool | ⏭️ SKIPPED | — (db.rs 698 lines, violates no-major-refactoring rule) |
| M-02 | DC IP configurable | ✅ | lib.rs |
| M-03 | WebDAV timing attack | ✅ | webdav_routes.rs |
| M-04 | X-Forwarded-For trust | ✅ | access_lockout.rs |
| M-05 | HMAC-SHA256 cookie | ✅ | share_routes.rs |
| M-06 | Rate limiter cleanup | ✅ | http_middleware.rs, server_http.rs |

## Phase 3: Frontend Architecture ✅ DONE

| Task | Status | Files Modified |
|------|--------|----------------|
| A11y: FileCard checkbox | ✅ | FileCard.tsx |
| A11y: ContextMenu | ✅ | ContextMenu.tsx |
| A11y: SettingsModal | ✅ | SettingsModal.tsx |
| A11y: MoveToFolderModal | ✅ | MoveToFolderModal.tsx |
| A11y: ShareDialog | ✅ | ShareDialog.tsx |
| A11y: UploadQueue | ✅ | UploadQueue.tsx |
| Responsive: SettingsModal | ✅ | SettingsModal.tsx |
| Responsive: MoveToFolderModal | ✅ | MoveToFolderModal.tsx |
| Responsive: ShareDialog | ✅ | ShareDialog.tsx |
| Responsive: Sidebar (mobile hamburger) | ✅ | Sidebar.tsx, Dashboard.tsx |
| React.memo: FileCard | ✅ | FileCard.tsx |
| React.memo: FileListItem | ✅ | FileListItem.tsx |
| React.memo: SidebarItem | ✅ | SidebarItem.tsx |
| React.lazy: AuthWizard + Dashboard | ✅ | App.tsx |

## Phase 4: Testing ✅ DONE

| Task | Status | Files |
|------|--------|-------|
| Vitest + jsdom config | ✅ | vitest.config.ts |
| Test setup (mocks) | ✅ | src/test-setup.ts |
| Utils unit tests | ✅ | src/utils.test.ts (16 tests) |
| UploadQueue component tests | ✅ | src/components/dashboard/UploadQueue.test.tsx (7 tests) |
| Existing tests | ✅ | src/lib/uploadPure.test.ts (3 tests) |
| **Total** | **26 tests, all passing** | |

## Phase 5: UI/UX ✅ DONE

| Task | Status | Files |
|------|--------|-------|
| SkeletonLoader component | ✅ | SkeletonLoader.tsx |
| Loading skeleton in FileExplorer | ✅ | FileExplorer.tsx |

## Phase 5: DevOps & Cleanup ✅ DONE

| Task | Status | Files |
|------|--------|-------|
| CHANGELOG v4.0.0-beta | ✅ | CHANGELOG.md |
| .gitignore AI directories | ✅ | .gitignore |

## Verification

- `cargo check --lib --features headless-server` ✅ PASS
- `cargo test --features headless-server` ✅ 61 tests pass
- `tsc --noEmit` ✅ PASS
- `vitest run` ✅ 26 tests pass

## Deferred / Remaining

- M-01: SQLite connection pool (requires db.rs rewrite)
- Component decomposition: AuthWizard (696L), SettingsModal (1231L), Dashboard (13 useState)
- i18n framework, search highlighting, file type filtering
- Docker optimization, CI/CD enhancement (cargo deny, trivy, lighthouse CI)
- UI animation improvements (stagger animations, modal transitions)
- Form validation enhancement
