# 快捷翻译桌面应用 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Windows-first personal desktop translator with a screenshot-to-OCR flow, a small manual translation window, replaceable online/local providers, and local history.

**Architecture:** Tauri 2 hosts a React/TypeScript UI and Rust application services. Rust owns platform capabilities, SQLite persistence, secure secret access, OCR, and provider networking; the frontend calls typed Tauri commands and listens for app events. Platform-specific operations remain behind traits so the domain services and provider routing can be tested without Windows.

**Tech Stack:** Tauri 2, Rust, React, TypeScript, Vite, Vitest, rusqlite, reqwest, keyring, Windows.Media.Ocr, Windows screen capture APIs, MyMemory, DeepL, and OpenAI-compatible translation endpoints.

**Spec:** `docs/superpowers/specs/2026-10-16-quick-translate-design.md`

## Global Constraints

- `桌面壳：Tauri 2。`
- `UI：TypeScript + React，负责小窗口、设置、历史记录和结果浮窗。`
- `系统能力：Rust，负责全局快捷键、系统托盘、截图、剪贴板、窗口管理和安全存储桥接。`
- `OCR：第一版优先使用 Windows 原生 OCR 适配器；保留 Tesseract/PaddleOCR 适配器接口作为跨平台或离线 fallback。`
- `翻译：定义统一 TranslationProvider 接口。内置免配置 provider（例如 MyMemory 或用户指定的 LibreTranslate 实例），并支持用户配置的 DeepL、Google、Microsoft、OpenAI 兼容接口等 provider。`
- `本地引擎：通过 LocalTranslationProvider 接入本机引擎；具体模型/运行时作为可选安装项，不强制进入首个安装包。`
- `配置与历史：本地 JSON/SQLite 均可；MVP 使用 SQLite 保存结构化历史，配置放在系统应用数据目录。`
- `密钥：Windows Credential Manager；跨平台时使用各系统原生钥匙串。`
- `“敏感内容”模式下只允许本地 provider；若本地引擎不可用，阻止上传并给出安装/配置提示。`
- `API Key 不出现在 UI 日志、异常信息或历史记录中。`
- `首版不做实时屏幕翻译、批量文档翻译、浏览器插件、云端同步、自动检测并替换原文、复杂术语库和账号体系。`

---

## File Structure

- `package.json`, `package-lock.json`, `vite.config.ts`, `tsconfig.json`, `index.html`: frontend toolchain and app entry.
- `src/main.tsx`, `src/App.tsx`, `src/styles.css`: React bootstrap, shell layout, and shared visual styles.
- `src/lib/commands.ts`, `src/lib/events.ts`: typed frontend wrappers around Tauri commands and events.
- `src/features/translate/`: manual translation form, result view, validation, and UI tests.
- `src/features/screenshot/`: screenshot-selection and OCR result UI.
- `src/features/settings/`: provider, language, privacy, shortcut, and history settings.
- `src/features/history/`: local history list and actions.
- `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`: Tauri and Rust configuration.
- `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`: app startup and command registration.
- `src-tauri/src/domain/`: language, translation request/result, provider config, and typed errors.
- `src-tauri/src/services/`: provider routing, translation orchestration, OCR, and screenshot flow.
- `src-tauri/src/providers/`: individual provider adapters.
- `src-tauri/src/platform/`: shortcut, screen capture, clipboard, tray, OCR, and secure-store implementations.
- `src-tauri/src/state.rs`: managed runtime state, including active screen-capture sessions and application services.
- `src-tauri/src/storage/`: settings and SQLite history repositories.
- `src-tauri/migrations/`: versioned SQLite schema migrations.
- `src-tauri/tests/`: Rust integration tests using fake providers and temporary databases.
- `tests/`: frontend component and interaction tests.
- `docs/manual/windows-acceptance.md`: repeatable manual checks for Windows-only behavior.

## Task 1: Scaffold the Desktop App and Test Harness

**Files:**
- Create: `package.json`, `package-lock.json`, `index.html`, `vite.config.ts`, `tsconfig.json`
- Create: `src/main.tsx`, `src/App.tsx`, `src/styles.css`
- Create: `src-tauri/Cargo.toml`, `src-tauri/build.rs`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`
- Create: `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`
- Create: `tests/app.test.tsx`

**Interfaces:**
- Frontend exports `App` from `src/App.tsx` and mounts it from `src/main.tsx`.
- Rust exports `run()` from `src-tauri/src/lib.rs`; `main.rs` invokes it.
- No translation behavior is added in this task.

- [ ] **Step 1: Add the Vite, React, Tauri, and Vitest project configuration**

Create a Vite React TypeScript project with Tauri 2 as the desktop shell. Add scripts named `dev`, `build`, `test`, `tauri`, and `check`; `check` runs `tsc --noEmit`, Vitest once, `cargo fmt --check`, `cargo test`, and the Vite production build in that order. Add `@testing-library/react`, `@testing-library/jest-dom`, `jsdom`, and Vitest for UI tests.

- [ ] **Step 2: Add the Rust crate and Tauri capability configuration**

Set the Rust package name to `quick_translate_lib`, configure the Tauri build script, and grant only the app capabilities needed for the main window, command invocation, and event listening. Do not grant unrestricted filesystem or shell access.

- [ ] **Step 3: Render the empty application shell**

Create an accessible app root with a title bar, navigation for `Translate`, `History`, and `Settings`, and an empty main panel. Keep dimensions stable and usable at a compact desktop window size.

- [ ] **Step 4: Add a smoke test for the shell**

In `tests/app.test.tsx`, render `<App />` and assert that the app title and all three navigation buttons are present. Configure Vitest with the `jsdom` environment and jest-dom matchers.

- [ ] **Step 5: Run the scaffold checks**

Run: `npm test -- --run`
Expected: the shell smoke test passes.

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: Rust compiles and the empty test suite passes.

Run: `npm run build`
Expected: Vite creates a production build without TypeScript errors.

- [ ] **Step 6: Commit the scaffold**

```bash
git add package.json package-lock.json index.html vite.config.ts tsconfig.json src src-tauri tests
git commit -m "chore: scaffold Tauri desktop app"
```

## Task 2: Define Translation Domain Types and Provider Contracts

**Files:**
- Create: `src-tauri/src/domain/mod.rs`
- Create: `src-tauri/src/domain/language.rs`
- Create: `src-tauri/src/domain/translation.rs`
- Create: `src-tauri/src/domain/provider.rs`
- Create: `src-tauri/src/domain/settings.rs`
- Create: `src-tauri/src/domain/error.rs`
- Create: `src-tauri/tests/domain_tests.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- `LanguageCode(String)` validates `^[A-Za-z]{2,3}(-[A-Za-z0-9]{2,8})*$` and serializes as a string; `auto` is not a valid selectable source language.
- `TranslationRequest { text: String, source: LanguageCode, target: LanguageCode }` rejects whitespace-only text and identical source/target values.
- `TranslationResult { translated_text: String, provider_id: String, elapsed_ms: u64, character_count: usize }` contains no credentials.
- `ProviderId(String)` identifies a provider.
- `ProviderConfig { id: ProviderId, display_name: String, endpoint: Option<Url>, model: Option<String>, secret_account: Option<String>, is_local: bool, enabled: bool }` stores a keyring account reference only, never a secret.
- `PrivacyMode` is `OnlinePreferred` or `LocalOnly`.
- `AppSettings { source_language: LanguageCode, target_language: LanguageCode, active_provider: ProviderId, privacy_mode: PrivacyMode, fallback_enabled: bool, history_enabled: bool, shortcut: String, providers: Vec<ProviderConfig> }` is the single persisted app configuration model.
- `TranslationProvider: Send + Sync` exposes `fn id(&self) -> &'static str` and async `translate(&self, request: &TranslationRequest) -> Result<TranslationResult, ProviderError>`; use `async_trait` so adapters can be stored as `Arc<dyn TranslationProvider>`.
- `ProviderError` variants are `Authentication`, `RateLimited { retry_after_seconds: Option<u64> }`, `Network`, `Timeout`, `Service { status: Option<u16> }`, `InvalidResponse`, `UnsupportedLanguage`, and `Configuration`.

- [ ] **Step 1: Write domain validation tests**

Test accepted codes `en`, `zh-CN`, and `ja`; reject an empty code, `auto`, and malformed codes. Test that non-empty text with different languages constructs a request, and that whitespace-only text or equal languages return a validation error.

- [ ] **Step 2: Run the domain tests and confirm they fail to compile**

Run: `cargo test --manifest-path src-tauri/Cargo.toml domain_tests`
Expected: compilation fails because the domain types do not exist yet.

- [ ] **Step 3: Add domain structs, serialization, and typed errors**

Implement the interfaces above with `serde`, `thiserror`, `async-trait`, and URL validation support. Keep provider errors free of request headers, response bodies, API keys, and raw secret values.

- [ ] **Step 4: Add a fake provider contract test**

Implement a test-only fake provider that returns a fixed result and verify its `id` and returned `provider_id` agree. Add an async test that propagates each `ProviderError` variant unchanged.

- [ ] **Step 5: Run Rust tests and formatting**

Run: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
Expected: no formatting changes are reported.

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: domain and fake-provider tests pass.

- [ ] **Step 6: Commit the domain contracts**

```bash
git add src-tauri/src/domain src-tauri/src/lib.rs src-tauri/tests/domain_tests.rs
git commit -m "feat: define translation domain contracts"
```

## Task 3: Persist Settings, History, and Secrets

**Files:**
- Create: `src-tauri/src/storage/mod.rs`
- Create: `src-tauri/src/storage/settings.rs`
- Create: `src-tauri/src/storage/history.rs`
- Create: `src-tauri/src/platform/secure_store.rs`
- Create: `src-tauri/migrations/001_initial.sql`
- Create: `src-tauri/tests/storage_tests.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- `SettingsRepository::load() -> Result<AppSettings, StorageError>` and `save(&AppSettings) -> Result<(), StorageError>` persist languages, active provider, privacy mode, history-enabled flag, and shortcut.
- `HistoryRepository::insert(&HistoryEntry) -> Result<i64, StorageError>`, `list(limit: u32) -> Result<Vec<HistoryEntry>, StorageError>`, `delete(id: i64)`, and `clear()` operate on SQLite.
- `SecureStore::set(service: &str, account: &str, secret: &str)`, `get(service, account) -> Result<Option<String>, SecureStoreError>`, and `delete(service, account)` delegate to the operating-system keyring.
- `HistoryEntry` stores `id`, source text, translated text, source/target codes, provider id, and UTC RFC3339 timestamp; it never stores a secret.

- [ ] **Step 1: Write SQLite repository tests against a temporary database**

Test that settings round-trip including the selected language pair and privacy mode. Test that history inserts, lists newest-first, deletes one entry, and clears all entries. Test that when history is disabled the translation orchestration can skip insertion.

- [ ] **Step 2: Run storage tests and confirm they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml storage_tests`
Expected: compilation fails because repository modules are not implemented.

- [ ] **Step 3: Create the initial SQLite migration and repository implementations**

Create a `settings` table keyed by `id = 1` and a `history` table with an integer primary key plus source, translation, languages, provider id, and timestamp columns. Add an index on descending timestamp. Use prepared statements and bound parameters for every query. Default `history_enabled` and `fallback_enabled` to true; sensitive mode always overrides fallback and blocks remote providers.

- [ ] **Step 4: Add keyring-backed secure storage**

Implement `SecureStore` using the `keyring` crate. Map OS/keyring errors into typed errors without formatting the supplied secret or raw platform credential data. Add tests with a fake secure-store implementation; do not write real credentials during automated tests.

- [ ] **Step 5: Run persistence tests and inspect the database boundary**

Run: `cargo test --manifest-path src-tauri/Cargo.toml storage_tests`
Expected: all temporary-database tests pass.

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: all existing Rust tests still pass.

- [ ] **Step 6: Commit persistence and secret storage**

```bash
git add src-tauri/src/storage src-tauri/src/platform/secure_store.rs src-tauri/migrations src-tauri/tests/storage_tests.rs
git commit -m "feat: add local settings history and secret storage"
```

## Task 4: Implement Provider Adapters and Privacy-Aware Routing

**Files:**
- Create: `src-tauri/src/providers/mod.rs`
- Create: `src-tauri/src/providers/mymemory.rs`
- Create: `src-tauri/src/providers/deepl.rs`
- Create: `src-tauri/src/providers/openai_compatible.rs`
- Create: `src-tauri/src/services/mod.rs`
- Create: `src-tauri/src/services/translation_router.rs`
- Create: `src-tauri/tests/provider_tests.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`

**Interfaces:**
- `ProviderId(String)` from Task 2 identifies `mymemory`, `deepl`, `openai-compatible`, or a configured local endpoint.
- `ProviderConfig` from Task 2 contains only a keyring account reference, never a secret.
- `ProviderRegistry::translate(provider_id: &ProviderId, request: &TranslationRequest) -> Result<TranslationResult, ProviderError>` dispatches to one registered adapter.
- `TranslationRouter::translate(mode: PrivacyMode, selected_provider: &ProviderId, request: &TranslationRequest, fallback_enabled: bool) -> Result<TranslationResult, TranslationError>` supports `OnlinePreferred` and `LocalOnly`. `LocalOnly` must never call an adapter where `is_local == false`.
- `TranslationError` is either `Provider(ProviderError)` or `NoLocalProvider`.
- MyMemory is the no-setup default and includes its attribution/limits notice in provider metadata. The UI identifies it as an online service before the first request.
- DeepL reads its API key from `SecureStore`; the OpenAI-compatible adapter supports configurable base URL, model, secret reference, and local endpoint mode.

- [ ] **Step 1: Write mocked-HTTP adapter tests**

Use a local mock HTTP server. Test successful response parsing for MyMemory and DeepL, bearer/key authentication header construction without exposing the key in errors, OpenAI-compatible chat-completion response parsing, non-2xx error classification, malformed JSON, and request timeouts.

- [ ] **Step 2: Write routing privacy tests**

Register one fake remote provider and one fake local provider. Assert `LocalOnly` calls only the local provider, errors when no local provider exists, and never calls the remote fake even if the local fake fails. Assert `OnlinePreferred` selects the configured online provider and only falls back to a configured local provider under the agreed fallback policy.

- [ ] **Step 3: Run provider tests and confirm they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml provider_tests`
Expected: compilation fails because the adapters and router are not implemented.

- [ ] **Step 4: Implement the MyMemory, DeepL, and OpenAI-compatible adapters**

Use `reqwest` with connection and total timeouts. Parse only expected response fields. Map 401/403 to `Authentication`, 429 to `RateLimited`, transport errors to `Network`/`Timeout`, other non-2xx responses to `Service`, and schema mismatches to `InvalidResponse`. Redact credentials and response bodies from errors.

- [ ] **Step 5: Implement provider registry and privacy-aware routing**

Keep selection deterministic. `LocalOnly` filters the registry before dispatch and returns a typed `NoLocalProvider` error when empty. In sensitive mode, do not perform automatic remote fallback under any circumstance. `OnlinePreferred` may fall back only to a provider marked local when `fallback_enabled` is true; the initial setting is true.

- [ ] **Step 6: Run the adapter, router, and full Rust test suites**

Run: `cargo test --manifest-path src-tauri/Cargo.toml provider_tests`
Expected: mocked responses and routing privacy tests pass.

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: all domain, storage, provider, and routing tests pass.

- [ ] **Step 7: Commit providers and routing**

```bash
git add src-tauri/src/providers src-tauri/src/services src-tauri/tests/provider_tests.rs src-tauri/src/lib.rs src-tauri/Cargo.toml
git commit -m "feat: add switchable translation providers"
```

## Task 5: Add Typed Tauri Commands and Manual Translation UI

**Files:**
- Create: `src-tauri/src/commands/mod.rs`
- Create: `src-tauri/src/commands/translation.rs`
- Create: `src-tauri/src/commands/clipboard.rs`
- Create: `src-tauri/src/platform/clipboard.rs`
- Create: `src/lib/commands.ts`
- Create: `src/features/translate/TranslateView.tsx`
- Create: `src/features/translate/TranslateView.test.tsx`
- Create: `src/features/translate/translate-state.ts`
- Modify: `src/App.tsx`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Tauri command `translate_text(text, source, target, provider_id) -> Result<TranslationResultDto, AppErrorDto>` validates input, loads settings/secrets, calls the router, and writes history only when enabled.
- Tauri command `list_languages() -> Vec<LanguageOptionDto>` returns the supported explicit source and target language choices; it does not include automatic detection in the MVP.
- Tauri command `list_providers() -> Vec<ProviderOptionDto>` returns display-safe provider id/name/local/configured/default metadata without secrets.
- Tauri command `write_clipboard(text) -> Result<(), AppErrorDto>` calls the platform `Clipboard` interface.
- Frontend wrapper exports `translateText`, `listLanguages`, and `listProviders` with exact DTO types.
- `TranslationResultDto { translatedText: string, providerId: string, elapsedMs: number, characterCount: number }` mirrors the display-safe result.
- `LanguageOptionDto { code: string, label: string }` and `ProviderOptionDto { id: string, displayName: string, isLocal: boolean, configured: boolean, isDefault: boolean, notice?: string }` populate selectors without secrets.
- `AppErrorDto { code: string, message: string, retryable: boolean }` is the only error shape returned to React.
- `TranslateView` exposes source and target selectors, a text area, translate command, output, copy action, and inline error state.

- [ ] **Step 1: Write frontend translation interaction tests**

Mock the Tauri command wrapper. Assert blank input does not invoke translation, valid input invokes `translateText` with selected languages/provider, success renders the translated text, and provider failure keeps the source text and shows a retryable error message.

- [ ] **Step 2: Add a Rust command-level test for orchestration**

Inject fake repositories and a fake provider into the translation service. Verify successful translation writes exactly one history entry when enabled, writes none when disabled, and never returns a secret-bearing field.

- [ ] **Step 3: Run the new UI and command tests and confirm they fail**

Run: `npm test -- --run src/features/translate/TranslateView.test.tsx`
Expected: the test fails because the view and typed wrapper do not exist.

Run: `cargo test --manifest-path src-tauri/Cargo.toml translation_command_tests`
Expected: the test fails to compile because the command orchestration is not implemented.

- [ ] **Step 4: Implement DTOs, commands, and injectable translation orchestration**

Register application services at startup and expose only serializable DTOs through Tauri commands. Convert internal errors to short user-facing codes/messages; do not serialize raw provider errors or secrets.

- [ ] **Step 5: Implement the manual translation view**

Use source and target language selects, an input area, provider selector, translate button, output region, copy button, and visible loading/empty/error states. Disable submission while pending and allow retry after failure. The copy button calls the typed `write_clipboard` wrapper.

- [ ] **Step 6: Run frontend and Rust command tests**

Run: `npm test -- --run src/features/translate/TranslateView.test.tsx`
Expected: input validation, invocation, success, copy, and error tests pass.

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: command orchestration and existing tests pass.

- [ ] **Step 7: Commit the manual translation workflow**

```bash
git add src-tauri/src/commands src-tauri/src/lib.rs src/lib/commands.ts src/features/translate src/App.tsx tests
git commit -m "feat: add manual translation window"
```

## Task 6: Implement Screenshot Selection and OCR

**Files:**
- Create: `src-tauri/src/platform/screen_capture.rs`
- Create: `src-tauri/src/platform/windows_ocr.rs`
- Create: `src-tauri/src/services/ocr.rs`
- Create: `src-tauri/src/commands/screenshot.rs`
- Create: `src/features/screenshot/ScreenshotOverlay.tsx`
- Create: `src/features/screenshot/ScreenshotResult.tsx`
- Create: `src/features/screenshot/screenshot.test.tsx`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/platform/mod.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- `ScreenCapture::capture_all_displays() -> Result<Vec<CapturedDisplay>, CaptureError>` captures each monitor and records its screen origin and scale.
- `CapturedDisplay { id: String, origin_x: i32, origin_y: i32, scale_factor: f64, preview_data_url: String, width: u32, height: u32 }` carries a JPEG preview and coordinate metadata to the overlay; raw pixel buffers stay in managed Rust state.
- `ImageRegion { rgba: Vec<u8>, width: u32, height: u32 }` is the cropped image passed from the capture service to OCR.
- `ScreenshotRect { x: u32, y: u32, width: u32, height: u32 }` uses physical pixels relative to its display.
- `OcrBlock { text: String, confidence: Option<f32>, bounds: ScreenshotRect }` reports recognized text and bounds.
- `OcrEngine::recognize(image: ImageRegion, source_language: Option<&LanguageCode>) -> Result<OcrResult, OcrError>` returns text, confidence, and text-block bounds.
- `OcrResult { text: String, confidence: Option<f32>, blocks: Vec<OcrBlock> }` allows editing OCR text before translation.
- Tauri command `begin_screenshot_translation() -> Result<Vec<CapturedDisplayDto>, AppErrorDto>` captures displays, stores pixel buffers in managed runtime state, and returns preview data URLs and display bounds.
- Tauri command `recognize_screenshot_region(display_id, rect) -> Result<OcrResultDto, AppErrorDto>` validates and crops the selected display region, then invokes the OCR service.
- Tauri command `cancel_screenshot_translation(display_id)` releases the stored pixel buffer for the selected display.
- `CapturedDisplayDto` mirrors display id, origin, scale factor, preview URL, width, and height; `OcrResultDto` mirrors recognized text, optional confidence, and block bounds.
- The frontend opens one full-screen overlay window per returned display. It invokes `recognize_screenshot_region` only after mouse-up and closes all overlay windows after OCR, cancellation, or an error.

- [ ] **Step 1: Write OCR service tests using a fake engine**

Test successful text and block return, whitespace-only OCR mapped to `EmptyResult`, unsupported language mapped to `LanguageUnavailable`, and OCR timeout mapped to `Timeout`. The tests must not call Windows APIs.

- [ ] **Step 2: Write screenshot UI tests**

Mock the OCR-result event. Assert the overlay accepts a selected rectangle, OCR text is editable, empty OCR shows a retry action, and translation starts only after the user confirms.

- [ ] **Step 3: Run OCR and screenshot UI tests and confirm they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml ocr_service_tests`
Expected: compilation fails because the OCR abstraction is absent.

Run: `npm test -- --run src/features/screenshot/screenshot.test.tsx`
Expected: tests fail because the screenshot components are absent.

- [ ] **Step 4: Implement the OCR service and a fake-backed platform boundary**

Keep image cropping, language selection, and error mapping in the service. Platform implementations only capture pixels or call OCR APIs. Never log image bytes or recognized text.

- [ ] **Step 5: Implement Windows screen capture and native OCR adapters**

Capture each monitor into a buffer with bounds and scaling metadata; encode a preview for the overlay and retain original pixels only in managed Rust state. Crop using physical display coordinates and reject zero-sized or out-of-bounds rectangles. Use Windows.Media.Ocr with the installed language pack. Return a typed language-pack error rather than silently changing the requested source language. Keep a platform-neutral `OcrEngine` boundary so a Tesseract/PaddleOCR implementation can be added without changing the service.

- [ ] **Step 6: Implement the overlay and OCR confirmation flow**

Show one full-screen transparent overlay per display over its JPEG preview. Use pointer drag to select a fixed rectangle; Escape cancels. Convert CSS coordinates to physical pixels using the display scale factor. Crop to the selected rectangle, invoke OCR, then show editable recognized text with source/target choices and an explicit Translate action. On cancel, error, or successful OCR, invoke the cancel/release command so screenshot buffers are not retained.

- [ ] **Step 7: Run platform-independent tests and build on Windows**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: fake OCR, provider, storage, and command tests pass.

Run: `npm test -- --run`
Expected: all UI tests pass.

Run: `npm run tauri -- dev`
Expected: a Windows development build starts, the screenshot overlay opens, Escape cancels, and selection returns editable OCR text.

- [ ] **Step 8: Commit screenshot OCR**

```bash
git add src-tauri/src/platform src-tauri/src/services/ocr.rs src-tauri/src/commands src-tauri/src/lib.rs src-tauri/Cargo.toml src/features/screenshot tests
git commit -m "feat: add screenshot OCR translation flow"
```

## Task 7: Add Settings, Provider Setup, History, Clipboard, and Tray Shortcut

**Files:**
- Create: `src-tauri/src/commands/settings.rs`
- Create: `src-tauri/src/commands/history.rs`
- Create: `src-tauri/src/platform/shortcut.rs`
- Create: `src-tauri/src/platform/tray.rs`
- Create: `src/features/settings/SettingsView.tsx`
- Create: `src/features/settings/SettingsView.test.tsx`
- Create: `src/features/history/HistoryView.tsx`
- Create: `src/features/history/HistoryView.test.tsx`
- Modify: `src/lib/commands.ts`
- Modify: `src/App.tsx`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/platform/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `docs/manual/windows-acceptance.md`

**Interfaces:**
- Tauri commands `get_settings`, `save_settings`, `save_provider_config`, `set_provider_secret`, `delete_provider_secret`, `list_history`, `delete_history_entry`, and `clear_history` expose typed settings/history operations.
- Saving a provider secret sends it once to Rust and persists it only through `SecureStore`; settings DTOs return only `configured: bool`.
- A provider marked local must use a loopback endpoint (`localhost`, `127.0.0.1`, or `[::1]`); Rust validates this before setting `is_local = true`.
- `GlobalShortcut::register(accelerator, callback)` reports invalid/conflicting shortcuts and does not silently replace another app's shortcut.
- Tray actions are `Open translator`, `Translate screenshot`, and `Quit`.
- `Clipboard::write_text(text)` is exposed through a command used by both manual and screenshot result views.

- [ ] **Step 1: Write settings and history UI tests**

Mock commands and test: changing source/target persists settings; changing privacy mode to sensitive updates visible provider options to local-only; adding an API key calls `set_provider_secret` but never renders it afterward; history-disabled state is saved; deleting and clearing history update the visible list.

- [ ] **Step 2: Write shortcut and secret command tests**

Use fake `GlobalShortcut` and `SecureStore` implementations. Verify a conflicting accelerator returns a readable error; secret set/get/delete delegates only to the secure store; serialized settings never include a secret value.

- [ ] **Step 3: Run these tests and confirm they fail**

Run: `npm test -- --run src/features/settings/SettingsView.test.tsx src/features/history/HistoryView.test.tsx`
Expected: tests fail because the views and commands are not implemented.

Run: `cargo test --manifest-path src-tauri/Cargo.toml shortcut_secret_command_tests`
Expected: tests fail to compile because platform command integration is absent.

- [ ] **Step 4: Implement typed settings/history commands and clipboard command**

Wire commands to the repositories and secure store. Ensure error conversion strips secrets and that turning history off prevents subsequent inserts while retaining existing entries until the user explicitly clears them.

- [ ] **Step 5: Implement settings UI and provider configuration**

Add default language pair, active provider, privacy mode, fallback preference, history toggle, shortcut input, provider endpoint/model fields, and provider credential forms. Render key status as `Configured`/`Not configured`, never as a masked echo of the stored value.

- [ ] **Step 6: Implement history UI and actions**

List the most recent 100 entries newest-first. Each row supports reopen, retranslate, and delete. Add a separate confirmation before clearing all history.

- [ ] **Step 7: Implement global shortcut and tray behavior**

Register the configured accelerator on startup, route it to `begin_screenshot_translation`, expose a menu item to open the translator, and exit cleanly on Quit. Show actionable errors when registration fails.

- [ ] **Step 8: Document Windows manual acceptance checks**

Write explicit steps for first launch, selecting a language pair, saving provider credentials, confirming credential absence in settings/config/logs, switching sensitive mode, translating from screenshot, retrying after network failure, turning history off/on, clearing history, changing the shortcut, and using each tray action.

- [ ] **Step 9: Run all UI and Rust tests**

Run: `npm test -- --run`
Expected: all translation, screenshot, settings, and history UI tests pass.

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: all domain, provider, storage, OCR, shortcut, and command tests pass.

- [ ] **Step 10: Commit settings and desktop workflow**

```bash
git add src-tauri/src/commands src-tauri/src/platform src-tauri/src/lib.rs src/lib/commands.ts src/features/settings src/features/history src/App.tsx docs/manual/windows-acceptance.md tests
git commit -m "feat: add settings history and desktop shortcuts"
```

## Task 8: Integrate, Package, and Verify the Windows MVP

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/capabilities/default.json`
- Modify: `package.json`
- Create: `README.md`
- Modify: `docs/manual/windows-acceptance.md`
- Create: `src-tauri/tests/app_integration_tests.rs`

**Interfaces:**
- `npm run check` runs frontend type checks, frontend tests, Rust formatting check, Rust unit/integration tests, and production build.
- `npm run tauri -- build` produces a Windows installer using configured product metadata and minimal capabilities.
- README documents prerequisites, install/run/build steps, provider setup, sensitive mode, and known free-provider limits.

- [ ] **Step 1: Add end-to-end service integration tests**

Build an application service with a temporary SQLite database, fake secure store, fake OCR engine, and mock provider server. Exercise manual translation and screenshot OCR confirmation; assert history policy and provider selection at the service boundary.

- [ ] **Step 2: Run the integration tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml app_integration_tests`
Expected: manual and screenshot service flows pass without live network access or credentials.

- [ ] **Step 3: Configure Windows bundle metadata and minimal permissions**

Set product name, identifier, version, icons, bundle targets, and app-data permissions. Remove any development-only capability grants before packaging.

- [ ] **Step 4: Write the user README**

Document Windows prerequisites; development and production commands; first-run MyMemory behavior; DeepL/OpenAI-compatible setup; local endpoint setup; sensitive mode guarantees; history retention controls; and that free service availability and quotas may change.

- [ ] **Step 5: Run the full automated verification command**

Run: `npm run check`
Expected: TypeScript, UI tests, Rust formatting, Rust tests, and frontend production build all pass.

- [ ] **Step 6: Build and inspect the Windows installer**

Run: `npm run tauri -- build`
Expected: the Windows installer is generated, installs without admin privileges if configured, launches the translator window, and shows the tray icon.

- [ ] **Step 7: Complete manual Windows acceptance**

Follow every step in `docs/manual/windows-acceptance.md`. Confirm screenshot selection works on single- and multi-monitor setups, OCR supports installed English and Chinese language packs, shortcut conflicts are surfaced, network failure remains recoverable, secrets are not exposed, and sensitive mode makes no remote requests.

- [ ] **Step 8: Commit the integrated MVP**

```bash
git add src-tauri/tauri.conf.json src-tauri/capabilities/default.json package.json README.md docs/manual/windows-acceptance.md src-tauri/tests/app_integration_tests.rs
git commit -m "feat: package Windows quick translate MVP"
```

## Spec Coverage Review

- Screenshot hotkey, region capture, OCR, editable source text, and empty-result recovery: Tasks 6 and 7.
- Manual translation, source/target selection, copy, loading/error states: Tasks 2 and 5.
- Online/local switching, no-setup MyMemory, user-configured DeepL and OpenAI-compatible providers, privacy routing, provider errors and timeouts: Task 4.
- Optional local engine path without bundling a model: Tasks 4 and 8.
- Persisted settings, SQLite history, deletion, clear, and disable-history behavior: Tasks 3 and 7.
- Secure key storage and secret redaction: Tasks 3, 4, and 7.
- Tray, global shortcut, clipboard, Windows-only manual acceptance, and packaging: Tasks 6, 7, and 8.
- Cross-platform boundaries: Tasks 2, 3, 4, and 6 keep domain services independent of Windows implementations.
