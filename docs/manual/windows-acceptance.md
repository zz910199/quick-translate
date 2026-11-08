# Windows manual acceptance

App-owned data must remain under `D:/QuickTranslate/Data` (SQLite, WebView data and packaged logs). API keys belong only in Windows Credential Manager under service `quick-translate`. Do not paste real credentials into screenshots, issue reports or terminal commands.

## First launch and translation

1. Start Quick Translate. Confirm the translator, History and Settings tabs load. Verify `D:/QuickTranslate/Data/quick-translate.sqlite3` exists. Restart and check saved preferences survive.
2. Open Settings. Select a different default source and target, choose an enabled service and save. Reopen Translate; verify both language choices. Enter a short phrase, translate, and copy the result into Notepad.
3. Configure DeepL with service ID `deepl`, name `DeepL`, and endpoint `https://api-free.deepl.com/v2/translate`. For an OpenAI compatible service use ID `openai-compatible` (or a unique custom ID), an HTTPS base endpoint ending in `/v1`, and a model name. Save the service first. Enter its API key and click 保存密钥. Confirm the input clears and only 已配置 is displayed. Reopen Settings and confirm no masked or plain key is restored. Delete the key and confirm 未配置.
4. Inspect serialized settings/config and app-owned logs without copying the key into a shell command. Confirm they contain endpoint/model/account references but no credential value. Windows Credential Manager should own the credential. Fake-keyring automated tests cover this boundary without requiring live secrets.
5. Configure a local OpenAI compatible service with a unique ID (for example `local-ollama`), endpoint `http://localhost:11434/v1`, a locally installed model, and 本地服务 enabled. Save, select 敏感模式 · 仅本地 and save settings. Confirm the default-service and Translate selectors contain only local services. A remote HTTPS endpoint marked local must be rejected. Also test `127.0.0.1` and `[::1]` endpoints. With no local service enabled, translation must explain that a local service is required.
6. Disconnect networking or stop the chosen server. Translate and verify a readable error with retry, preserving source text. Restore networking/server, retry, and confirm a result can be copied. Confirm sensitive mode never sends text to online fallback services.

## Packaging and D-drive paths

1. Set `CARGO_HOME`, `RUSTUP_HOME`, `TEMP`/`TMP`, npm cache and the build process's `LOCALAPPDATA` to the D-drive locations in README. Run `npm run check`, then `npm run tauri -- build`. Confirm an NSIS installer appears in the project's `src-tauri/target/release/bundle/nsis` directory on D.
2. Ask the user to confirm the application install directory before running the installer. Run it interactively, select the confirmed D-drive directory at the directory page, and verify current-user installation needs no elevation. The installer intentionally does not install WebView2; verify the existing Runtime prerequisite first.
3. Launch the installed program and verify the translator window and tray icon appear. Verify executable and uninstaller live at the selected D-drive path, SQLite at `D:/QuickTranslate/Data/quick-translate.sqlite3`, and WebView files under `D:/QuickTranslate/Data/WebView`. The app must not silently fall back to C when D is unavailable.
4. Verify project `dist`, Rust `target`, Rust toolchain/cache, npm cache, bundler cache and temporary files stay on D. Windows-managed Credential Manager, registry and Start Menu entries remain system-managed; they are not app-owned data directories.

## Screenshot and cancellation

1. Click 截图翻译, select a text region, edit the OCR confirmation, verify source/target choices and press 翻译. Confirm recognized text enters the translator; press 翻译 there to send it. Copy the final result using 复制译文.
2. Press Escape while an overlay is open; confirm all overlays close. Reopen and cancel while OCR is pending, then start a new capture. No old result/error may replace the new session.
3. Select a blank area and verify the empty-result retry path. Test an unavailable Windows OCR language pack and confirm an actionable message.
4. On a multi-display host, repeat with different scaling factors, monitors left/above the primary, and each display. Verify physical selection alignment and session-wide cancellation. Single-display testing does not establish this acceptance.

## History

1. Enable 保存翻译历史, save, translate, and confirm the newest entry appears first. Reopen it; verify original pair, source and result, editing source, 复制译文 and 重新翻译.
2. Disable history and save. Translate again; confirm no new entry is stored and existing rows remain. Enable history and confirm new results resume saving.
3. Delete one row and verify it disappears. Click 清空历史, then 取消; confirm records remain. Click 清空历史 then 确认清空 and confirm the list is empty after restarting.
4. With more than 100 stored rows, confirm the view shows the newest 100; clearing removes all stored records, including older hidden rows.

## Shortcut and tray

1. Press the default `Ctrl+Shift+T` from another app. Confirm it opens the same screenshot/OCR workflow and returns recognized text to the main window.
2. Change 截图快捷键 to an available combination and save. Confirm the new key works immediately and the old one no longer starts capture. Restart and verify the new setting.
3. Enter an invalid accelerator or one owned by another application. Save must show an actionable error and preserve the previous working key/settings. Start the app while its configured key is already occupied: the translator should remain usable and display an error with an 打开设置 action.
4. Use each tray menu action: 打开翻译器 restores/focuses the main window; 截图翻译 follows the same session workflow; 退出 closes the application and releases its global shortcut. Confirm no app process remains after Quit. Repeated quick shortcut presses during capture startup should not create competing startup requests.

## Window lifecycle

1. Open the translator, then close it with the title-bar ×. Confirm the process survives in Task Manager, the tray icon stays, and the global shortcut still starts a capture. Reopen from the tray.
2. Use 退出 in the tray menu. Confirm the process ends, the tray icon disappears and the shortcut is released.

## Optimization wave (2026-11-08)

1. Translate a long Chinese paragraph with MyMemory selected. Confirm the counter reads bytes (`x / 500 字节`) rather than characters, turns red past the limit, and the translate button is disabled instead of failing after a request.
2. Delete the currently selected service in Settings. Confirm the confirmation appears first, the configuration disappears, and the default service falls back to a remaining one. Confirm no `provider:<id>` credential is left in Credential Manager.
3. Use 重新翻译 on a history row. Confirm the row is updated in place, the list still holds the same number of entries, and 已更新这条记录 appears. Then delete the service that entry used and retranslate again: the row must still update, using the current default service, with a message naming it.
4. With a local provider stopped (or networking off), translate once. Confirm one automatic retry happens before the error appears, and the error still offers 重试.
5. Press the screenshot shortcut on a high-resolution or multi-display setup. Confirm the overlay still aligns with the physical selection and appears for all displays together rather than one after another. The preview is downscaled to 2560 px on the long edge; OCR still uses the full-resolution capture.

## Acceptance status

Task 8 automated verification (`npm run check`) passed on 2026-10-17: 26 UI tests, 59 Rust tests and 2 compile-fail doctests. This includes real loopback HTTP + temporary SQLite application service integration for manual translation, history policy and edited OCR confirmation routed to a local provider in sensitive mode. No live credentials or external services are needed for these tests.

The 2026-11-08 optimization wave re-ran `npm run check` with 30 UI tests, 73 Rust tests and the same 2 compile-fail doctests, plus a clean production build. New coverage: in-place history retranslation, provider fallback for removed services, single transient retry, registry cache invalidation after a configuration change, provider deletion with reselection, byte-limit metadata, preview downscaling, and concurrent repository opening on one SQLite file. Everything in the "Window lifecycle" and "Optimization wave" sections above is unverified on the packaged application.


The final `npm run tauri -- build` passed on 2026-10-17 and produced `Quick Translate_0.1.0_x64-setup.exe` (3,825,248 bytes; SHA-256 `BB5DFAE3C006B0AA5EA1BF765DA07470A709E4625D222F1D6CAB0331D5225E2E`). Generated NSIS source confirms `currentUser`, `RequestExecutionLevel user` and an interactive directory page. The installer is unsigned. Bundler cache was verified under the project's D-drive `src-tauri/target/.tauri/NSIS`, with no `C:/Users/july/AppData/Local/tauri` directory after the final build. These build/source checks do not establish successful installation.

Native installation is intentionally pending the user's installation-path confirmation. Current-user/no-elevation installation, installed launch/tray, real Credential Manager interaction, operating-system shortcut conflicts, live network recovery and live provider behavior are still unverified. Native tray/shortcut and cancellation fixes from Task 7 must also be exercised on the packaged application. The earlier Task 6 report records successful single-monitor overlay, Escape, Chinese OCR and edit/confirm tests, plus the expected English language-pack error. It does not establish multi-monitor/mixed-DPI correctness or successful English OCR; those remain pending the required hardware/language pack. Automated checks do not convert these pending items into passes.
