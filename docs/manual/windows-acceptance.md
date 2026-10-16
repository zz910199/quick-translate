# Windows manual acceptance

App-owned data must remain under `D:/QuickTranslate/Data` (SQLite, WebView data and packaged logs). API keys belong only in Windows Credential Manager under service `quick-translate`. Do not paste real credentials into screenshots, issue reports or terminal commands.

## First launch and translation

1. Start Quick Translate. Confirm the translator, History and Settings tabs load. Verify `D:/QuickTranslate/Data/quick-translate.sqlite3` exists. Restart and check saved preferences survive.
2. Open Settings. Select a different default source and target, choose an enabled service and save. Reopen Translate; verify both language choices. Enter a short phrase, translate, and copy the result into Notepad.
3. Configure DeepL with service ID `deepl`, name `DeepL`, and endpoint `https://api-free.deepl.com/v2/translate`. For an OpenAI compatible service use ID `openai-compatible` (or a unique custom ID), an HTTPS base endpoint ending in `/v1`, and a model name. Save the service first. Enter its API key and click 保存密钥. Confirm the input clears and only 已配置 is displayed. Reopen Settings and confirm no masked or plain key is restored. Delete the key and confirm 未配置.
4. Inspect serialized settings/config and app-owned logs without copying the key into a shell command. Confirm they contain endpoint/model/account references but no credential value. Windows Credential Manager should own the credential. Fake-keyring automated tests cover this boundary without requiring live secrets.
5. Configure a local OpenAI compatible service with a unique ID (for example `local-ollama`), endpoint `http://localhost:11434/v1`, a locally installed model, and 本地服务 enabled. Save, select 敏感模式 · 仅本地 and save settings. Confirm the default-service and Translate selectors contain only local services. A remote HTTPS endpoint marked local must be rejected. Also test `127.0.0.1` and `[::1]` endpoints. With no local service enabled, translation must explain that a local service is required.
6. Disconnect networking or stop the chosen server. Translate and verify a readable error with retry, preserving source text. Restore networking/server, retry, and confirm a result can be copied. Confirm sensitive mode never sends text to online fallback services.

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

## Acceptance status

Task 7 automated tests use fake secure storage and shortcut registration. Native tray clicks, operating-system shortcut conflicts, close/reopen behavior and real Credential Manager interaction still require manual verification on Windows. Earlier Task 6 OCR manual observations and multi-display limitations are recorded in its report.
