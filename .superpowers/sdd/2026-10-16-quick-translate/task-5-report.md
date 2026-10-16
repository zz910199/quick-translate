# Task 5 Report: Typed Commands and Manual Translation UI

## Status

Implemented and verified. The work adds typed Tauri commands, injectable runtime services, display-safe DTOs, a Windows clipboard boundary, D-drive startup persistence, and the manual Chinese-first translation view.

## TDD evidence

RED:

```text
npm test src/features/translate/TranslateView.test.tsx
Error: Failed to resolve import "../../lib/commands" ... the view and typed wrapper did not exist.

.superpowers/sdd/2026-10-16-quick-translate/run-task3-check.cmd test --manifest-path src-tauri/Cargo.toml translation_command_tests
error[E0433]: could not find `commands` in `quick_translate_lib`
error[E0432]: could not find `platform::clipboard`
```

GREEN focused:

```text
npm test src/features/translate/TranslateView.test.tsx
Test Files 1 passed; Tests 7 passed

.superpowers/sdd/2026-10-16-quick-translate/run-task3-check.cmd test --manifest-path src-tauri/Cargo.toml translation_command_tests
test result: ok. 3 passed; 0 failed
```

Full verification:

```text
npm test
Test Files 2 passed; Tests 9 passed
npm run build
tsc --noEmit and vite build passed
.superpowers/sdd/2026-10-16-quick-translate/run-task3-check.cmd test --manifest-path src-tauri/Cargo.toml
12 domain + 16 provider + 8 storage + 3 command + 2 doctests passed
.superpowers/sdd/2026-10-16-quick-translate/run-task3-check.cmd fmt --manifest-path src-tauri/Cargo.toml
passed
git diff --check
passed (only existing Windows LF/CRLF advisories)
```

## Implemented interfaces

- `src-tauri/src/commands/translation.rs` exports `AppServices`, `AppErrorDto`, `TranslationResultDto`, `LanguageOptionDto`, `ProviderOptionDto`, `TranslationSelectionDto`, `list_languages`, `list_providers`, `translate_text`, `get_translation_selection`, and `save_translation_selection`.
- `AppServices::new(settings, history, secure_store, clipboard)` accepts `Send + Sync` repositories and platform services. `with_registry_factory` injects a fake registry/provider for orchestration tests.
- `translate_text` validates language choices, blank/equal language input, MyMemory's 500 UTF-8 byte limit, effective LocalOnly provider selection, then rebuilds the provider registry from current settings. Secrets are fetched by adapters per request and never enter DTOs.
- History insertion is conditional on the current settings after the await, so disabling history during a pending request prevents the insert.
- `src/lib/commands.ts` exports typed wrappers: `translateText`, `listLanguages`, `listProviders`, `writeClipboard`, `getTranslationSelection`, and `saveTranslationSelection`.
- `TranslateView` accepts optional `draftText` for Task 6 OCR confirmation/screenshot toolbar seeding. It does not auto-submit the draft. Selection persistence is serialized and can be used by Task 7 settings controls.

## Data and privacy

Startup creates `D:/QuickTranslate/Data` and opens `D:/QuickTranslate/Data/quick-translate.sqlite3`; there is no AppData fallback. Tauri app directory override is also set to `D:/QuickTranslate/Data`. The LocalOnly selector filters out online providers and the router performs one local dispatch without fallback to online.

## Changed files

`src-tauri/src/commands/mod.rs`, `src-tauri/src/commands/translation.rs`, `src-tauri/src/commands/clipboard.rs`, `src-tauri/src/platform/clipboard.rs`, `src-tauri/src/state.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/platform/mod.rs`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/tauri.conf.json`, `src-tauri/tests/translation_command_tests.rs`, `src/lib/commands.ts`, `src/features/translate/TranslateView.tsx`, `src/features/translate/TranslateView.test.tsx`, `src/features/translate/translate-state.ts`, `src/App.tsx`, `src/styles.css`, and `tests/app.test.tsx`.

## Concerns and handoff

- The manual view keeps the existing History and Settings shell routes as empty panels; Task 7 should mount its settings UI there and reuse `getTranslationSelection`/`saveTranslationSelection` plus `listProviders`.
- Task 6 can render `<TranslateView draftText={confirmedText} />` from its screenshot/OCR flow. A future toolbar action can also pass a new draft via the prop; the component intentionally leaves the request pending until the user presses 翻译.
- `arboard` is a thin Windows platform boundary. Clipboard failures are redacted to a retryable `clipboard` DTO.
