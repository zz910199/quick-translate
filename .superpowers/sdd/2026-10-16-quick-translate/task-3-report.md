# Task 3 report: persistence and secret storage

Status: DONE

## Implementation

- Added injectable `SettingsRepository` and `HistoryRepository` traits, with `SqliteSettingsRepository::open(path)` and `SqliteHistoryRepository::open(path)` concrete implementations. Both concrete types are `Send + Sync` through a mutex-protected SQLite connection.
- Every repository accepts an explicit database path. No AppData directory lookup or app-owned C: path was introduced. Startup can create `D:/QuickTranslate/Data` and open its database there in the later startup task; the containing directory must exist before `open`.
- Initial migration atomically creates a singleton settings table (`id = 1`), history table, and descending timestamp/id index. `BEGIN IMMEDIATE`, idempotent DDL and a five-second SQLite busy timeout support concurrent repository initialization against one database.
- Settings use a JSON payload of the validated domain `AppSettings`, preserving all fields including provider configuration/account references. Persisted malformed settings yield `StorageError::InvalidData` rather than silently resetting user configuration.
- Defaults: English `en` to `zh-CN`; provider id `mymemory`, display name `MyMemory`, endpoint `https://api.mymemory.translated.net/get`; `OnlinePreferred`; history and fallback enabled; shortcut `Ctrl+Shift+T`.
- History supports insert, limited newest-first list, delete by id, and clear. Entries retain source text, translation, languages, provider id and UTC timestamp, without any credential field. UTC timestamps are stored as fixed-width nanosecond RFC3339 strings ending in `Z`, so SQLite text ordering and chronological ordering agree. Equal timestamps sort by descending generated id.
- Every variable query value uses a bound parameter. The migration contains only trusted static SQL. Database errors deliberately omit raw SQLite errors, which may contain stored user text.
- Added `SecureStore: Send + Sync` and `KeyringSecureStore`, whose default backend uses `keyring::Entry` and the OS credential store. Keyring is configured with Windows native, Apple native and synchronous Secret Service backends. No app-owned secret file is created. A separate injectable `CredentialBackend` allows testing the production secure-store facade without writing real OS credentials.
- Secure-store missing reads return `None`; deleting a missing credential succeeds. Access, invalid-input and other platform failures map to typed, redacted errors that retain neither supplied secrets nor raw platform error payloads.
- The history-enabled flag is persisted and available to later orchestration. Disabling it preserves saved history. As directed by the controller, the actual skip-insertion integration belongs to Task 5; privacy/fallback routing enforcement belongs to Task 4. No throwaway orchestration was added to manufacture those tests.

## TDD evidence

Tests were written before production modules/dependencies were added.

RED command:

```text
.superpowers/sdd/2026-10-16-quick-translate/run-task3-check.cmd test --manifest-path src-tauri/Cargo.toml storage_tests
```

Expected failure, exit 1:

```text
error[E0433]: cannot find `platform` in `quick_translate_lib`
error[E0432]: unresolved import `quick_translate_lib::storage`
error[E0432]: unresolved import `chrono`
error[E0433]: cannot find module or crate `rusqlite` in this scope
error: could not compile `quick_translate_lib` (test "storage_tests") due to 21 previous errors
```

The missing modules and dependencies were the intended reason for failure, consistent with the brief's compile-failing RED gate.

GREEN command (same command as RED), exit 0:

```text
running 8 tests
test storage_tests_secure_store_redacts_platform_errors_and_supplied_secrets ... ok
test storage_tests_secure_store_handles_missing_credentials_and_namespaces_accounts ... ok
test storage_tests_repositories_support_shared_runtime_ownership_and_concurrent_open ... ok
test storage_tests_settings_defaults_and_round_trip_survive_reopening ... ok
test storage_tests_corrupt_settings_return_a_typed_error ... ok
test storage_tests_history_disabled_flag_is_available_without_deleting_saved_history ... ok
test storage_tests_history_deletes_one_entry_and_clears_all ... ok
test storage_tests_history_persists_all_fields_orders_by_time_and_limits ... ok
test result: ok. 8 passed; 0 failed
```

The real temporary SQLite databases exercise save/reopen persistence, selected language/privacy/provider configuration, history ordering/limits, Unicode/SQL-like input, deletes/clear, concurrent migration, malformed settings, and saved-history preservation while disabled. Secret tests run the real `KeyringSecureStore` facade over fake external credentials, exercise separate services/accounts and updates, and check that supplied secrets/raw platform payloads do not appear in errors.

## Final verification

After formatting and self-review, ran:

```text
.superpowers/sdd/2026-10-16-quick-translate/run-task3-check.cmd test --manifest-path src-tauri/Cargo.toml
```

Exit 0, no compiler warnings:

```text
domain_tests: 12 passed; 0 failed
storage_tests: 8 passed; 0 failed
Doc-tests quick_translate_lib: 2 passed; 0 failed
```

Also ran:

```text
.superpowers/sdd/2026-10-16-quick-translate/run-task3-check.cmd fmt --manifest-path src-tauri/Cargo.toml -- --check
git diff --check
```

Both exited 0. Git reported only the repository's existing Windows LF-to-CRLF conversion notices, no whitespace errors.

The checked-in task command initializes MSVC and the D: Rust toolchain and sets process-local `TEMP`/`TMP` to `D:/DevTools/Temp`. Cargo downloads/cache remain under `D:/DevTools/Rust/cargo`; compilation remains in the D: worktree target directory. No machine-wide environment variables were changed.

## Files changed

- `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`
- `src-tauri/src/lib.rs`
- `src-tauri/src/storage/mod.rs`
- `src-tauri/src/storage/settings.rs`
- `src-tauri/src/storage/history.rs`
- `src-tauri/src/platform/mod.rs`
- `src-tauri/src/platform/secure_store.rs`
- `src-tauri/migrations/001_initial.sql`
- `src-tauri/tests/storage_tests.rs`
- `.superpowers/sdd/2026-10-16-quick-translate/run-task3-check.cmd`
- `.superpowers/sdd/2026-10-16-quick-translate/task-3-report.md`

## Self-review and limits

Reviewed all new modules, test behavior, migration, dependency changes and staged scope. Confirmed settings deserialize through validated domain types, history carries no credential field, all untrusted query values are bound, database connection locks cover insertion plus `last_insert_rowid`, and the module interfaces support later injected runtime services. No unrelated source changes.

No unresolved implementation concerns. Actual Windows credential-store writes were intentionally not performed in automated tests; the OS adapter is compile-checked and facade behavior is tested through the fake credential boundary. Apple/Linux runtime backends were not exercised on this Windows host. Routing enforcement, history orchestration, and creating/wiring the chosen D: startup directory remain with their later assigned tasks.
