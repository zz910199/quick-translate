# 现有功能优化方案（0.1.0 之后）

本方案不加新功能，只处理 MVP 已交付功能里的**稳定性缺陷、性能浪费和体验断点**。所有结论都基于对当前代码（分支 `quick-translate`，HEAD `031e135`）的阅读，证据标注到文件与行号。

## 1. 现状基线

| 项目 | 状态 |
| --- | --- |
| 前端类型检查 | `tsc --noEmit` 通过 |
| 前端测试 | 5 文件 / 26 用例通过 |
| 后端测试 | 63 用例 + 2 编译失败 doctest（上次报告数据，本轮未跑） |
| 打包 | NSIS 0.1.0 已成功产出，未执行安装 |
| 代码分层 | `domain / providers / services / storage / platform / commands` 边界清晰，fake 注入点齐全，可直接沿用 |

分层和测试基础设施是够用的，问题集中在**编排层（`commands/`）每次调用都重做初始化**、**桌面壳的生命周期没接完整**、以及**少数 UI 与后端约束不一致**。

## 2. 问题清单

优先级：P0 = 影响功能可用性或必然报错；P1 = 明显性能或体验损失；P2 = 打磨项。

### P0-1 关闭主窗口会退出进程，托盘与全局快捷键一起失效

- 证据：`src-tauri/src/lib.rs` 全文没有 `on_window_event` / `prevent_close` / `RunEvent::ExitRequested`；而 `platform/tray.rs` 提供了"打开翻译器 / 截图翻译 / 退出"三项菜单，说明设计意图是常驻托盘。
- 影响：Tauri 2 在最后一个窗口关闭时默认结束进程。点窗口右上角 ×，托盘图标消失、`Ctrl+Shift+T` 失效，必须重新启动程序——对一个"按快捷键随手翻译"的工具是硬伤。
- 建议：`main` 窗口监听 `CloseRequested` → `api.prevent_close()` + `window.hide()`；只有托盘"退出"走 `app.exit(0)`；首次隐藏时用托盘气泡提示"程序仍在后台运行"。
- 验证：`npm run tauri -- dev`，点 × 后在任务管理器确认进程存活，再按快捷键确认仍能触发截图；托盘"退出"后确认无残留进程。
- 改动面：`lib.rs` 约 15 行 + 一条 UI 测试无关的说明文档。

### P0-2 历史记录"重新翻译"沿用已失效的 provider，必然报错

- 证据：`src/features/history/HistoryView.tsx:32` 与 `:38` 把 `entry.providerId` 直接传给 `translateText`；`commands/translation.rs:211` 用该 ID 在 registry 里查找，不在则 `ProviderError::Configuration`，`provider_error()` 返回"翻译服务配置不可用，请在设置中检查。"
- 影响：用户删掉或禁用某个服务后，所有使用过该服务的旧记录点"重新翻译"都失败，且提示完全指错方向（配置其实没坏，是这条记录引用了旧服务）。
- 建议：后端在 `provider_id` 不在 registry 时回退到 `settings.active_provider`（仍需满足 `LocalOnly` 只能本地），并在 DTO 里返回实际使用的 provider；前端在两者不一致时提示"已改用当前服务 · {name}"。
- 验证：在 `src-tauri/tests/` 的翻译命令测试里加一例"历史 provider 已删除 → 回退并成功"。
- 改动面：`commands/translation.rs` 约 20 行 + 测试 1 例。

### P0-3 MyMemory 的 500 字节限制与界面上的"字符数"不一致

- 证据：`commands/translation.rs:224` 用 `request.text().len() > 500`（UTF-8 **字节**）；`src/features/translate/TranslateView.tsx:79` 显示的是 `Array.from(text).length`（**字符**）。
- 影响：中文一个字 3 字节，约 167 字就超限，用户看到计数"300 字符"却被告知"每次最多支持 500 UTF-8 字节"，无法自行预判，属于可复现的挫败点。
- 建议：把每服务上限作为 provider 元数据下发（`limitBytes`），前端显示"字节 x / 500"，接近上限即变色提示，超限时直接禁用翻译按钮并说明原因；后端保留现有硬校验作为兜底。
- 验证：`TranslateView.test.tsx` 增一例中文长文本的按钮禁用与提示。
- 改动面：`commands/translation.rs` 下发字段 + `TranslateView.tsx` 计数逻辑，约 30 行。

### P0-4 设置页只能新增服务，不能删除

- 证据：`src/features/settings/SettingsView.tsx` 只有"保存服务 / 保存密钥 / 删除密钥"；`src-tauri/src/commands/settings.rs` 无删除配置的 command，`invoke_handler` 也没有对应项。
- 影响：试错留下的服务配置永久堆积，`provider:{id}` 的凭据也无法清理（只能删密钥，账户名残留）；服务 ID 被占用后无法复用。
- 建议：新增 `delete_provider_config`：移除配置 → 清理对应 keyring 条目 → 若删的是 `active_provider` 则回退到第一个可用服务；UI 加"删除服务"并二次确认。
- 验证：`settings` 命令测试加"删除后配置与凭据都不存在、active 已回退"。
- 改动面：`commands/settings.rs` 约 30 行 + `commands/history.rs` 无需改动 + 前端约 20 行。

### P1-5 每次翻译都重建 ProviderRegistry

- 证据：`commands/translation.rs:211`，`translate_text` 内 `(self.registry_factory)(&settings.providers, ...)`；`list_providers` 同样每次重建（`:154`）。
- 影响：一次翻译要重新解析全部配置、为每个启用服务新建 `reqwest::Client`（`providers/mod.rs:205-222`，含 rustls 根证书加载）、并在 `configured` 判断里逐项读 Windows 凭据管理器。这些都与"文本是什么"无关，纯属固定开销，且直接叠加在用户等待的延迟上。
- 建议：`AppServices` 内缓存 `ProviderRegistry` + 一个配置指纹（序列化后哈希）；`save_provider_config` / `save_settings` / `set_provider_secret` 成功后失效缓存。保留 `with_registry_factory` 作为测试注入点，缓存逻辑对新接口透明。
- 度量方式：先在 `translate_text` 内打点记录 `registry_build_ms` 与总耗时（仅日志，不进 UI），改前改后各测 10 次冷/热请求对比，用数据确认收益再决定是否保留日志。
- 改动面：`commands/translation.rs` 约 40 行（新增 `Mutex<Option<CachedRegistry>>`），需补一条"配置变更后立即生效"的测试，避免缓存吃掉新配置。

### P1-6 单次翻译读两次设置

- 证据：`commands/translation.rs:210` 与 `:244`，第二次是 await 之后重读以让"翻译过程中关闭历史"生效（语义正确，但代价是又一次 SQLite 查询 + JSON 反序列化）。
- 建议：给 `SettingsRepository` 加一个轻量的 `history_enabled()`（`SELECT json_extract(payload,'$.history_enabled')` 或单独列），把"await 后重读"的成本降到单字段查询。语义保持不变。
- 改动面：`storage/settings.rs` + 测试 1 例，约 25 行。

### P1-7 截图预览按全分辨率编码

- 证据：`platform/screen_capture.rs:63-79`（`encode_preview` 用原始 width/height 逐像素拷 RGB 后 JPEG 质量 75）与 `:302`；`services/ocr.rs:143-155` 又把 `preview_data_url` 克隆一次交给前端。
- 影响：4K 单屏 RGBA 33 MB，转 RGB 再 JPEG 再 base64，得到 2–4 MB 的字符串经 IPC 送进 WebView；多屏时 `capture_all_displays` 还把所有屏幕的图像**同时**留在内存里。这是"按下快捷键到浮层出现"这一段的主要耗时。
- 建议：预览按最长边 2560 降采样（浮层是把图拉伸铺满窗口，视觉无差异，OCR 用的仍是原始 `rgba`，精度不受影响）；把 BGR→RGB 交换与 RGB 打包合并为一次遍历，同时写出预览缓冲。
- 验证：`preview_tests` 增一条"大图降采样后尺寸符合上限且可解码"；人工对比浮层对齐（多屏混合 DPI 本就是待验收项）。
- 改动面：`screen_capture.rs` 约 40 行。

### P1-8 浮层窗口在每个显示器上串行创建

- 证据：`commands/screenshot.rs:133-193`，`for display in &dtos` 内同步 `WebviewWindowBuilder::new(...).build()`。每建一个 WebView 实例是百毫秒级操作，三屏就是三次串行等待，用户看到浮层一个一个出现。
- 建议：两条路径二选一——(a) 并行创建：每屏 `tauri::async_runtime::spawn` 建窗，再汇总结果；(b) 预热：启动时创建隐藏浮层并复用，每次仅 `set_position` / `set_size` / 更新预览。
- 风险：(b) 与现有 generation/取消语义（`is_current` / `is_finished` / `cancel_session`）耦合更紧，需要新增测试覆盖"创建期间取消"和"上一次会话残留浮层"；因此建议先做 (a)，(b) 视实测收益再定。
- 改动面：(a) 约 50 行 + 测试 1–2 例。

### P2-9 `Retry-After` 被解析后丢弃，429 只能提示"稍后重试"

- 证据：`providers/mod.rs:258-262` 解析 `Retry-After` 存入 `ProviderError::RateLimited { retry_after_seconds }`；`commands/translation.rs:296` 用 `RateLimited { .. }` 忽略该值。
- 建议：`AppErrorDto` 增可选 `retryAfterSeconds`，UI 显示"约 N 秒后重试"并在此期间禁用重试按钮。可选再加一次自动重试：仅对 `Network` / `Timeout` 生效，指数退避 300 ms 起，最多 1 次——翻译是幂等只读操作，安全。
- 改动面：DTO 1 字段 + 两处 UI，约 30 行。

### P2-10 设置页改动不保存就切页，静默丢失

- 证据：`SettingsView.tsx` 全部字段是本地 `useState`，只有"保存设置"落盘；`TranslateView.tsx:29-39` 只在挂载时读一次选择。
- 影响：改了隐私模式或默认服务后切到翻译页，界面还用旧值且无任何提示，用户会以为设置没生效（实际上是没保存）。
- 建议：加脏标记并在切换视图/关闭窗口时提示；或把"隐私模式、默认服务、历史开关"改为即时保存（与翻译页下拉框已有的即时保存行为一致），"快捷键"仍保留显式保存（因为它会触发注册回滚逻辑）。
- 改动面：`SettingsView.tsx` + `App.tsx` 视图切换处，约 40 行。

### P2-11 翻译页缺常用键盘操作与语言互换

- 证据：`TranslateView.tsx` 无 `onKeyDown`，无互换按钮。
- 建议：`Ctrl+Enter` 触发翻译、输入框内 `Esc` 收起错误提示、语言选择器中间加 `⇄` 互换。都属于低成本高频操作。

### P2-12 历史列表无搜索，也无法把记录送回翻译页

- 证据：`HistoryView.tsx:36` 直接渲染 100 条，无过滤；详情只有"复制译文 / 重新翻译 / 关闭"。
- 建议：前端内存搜索（原文、译文、服务 ID、语言对），零后端改动；详情加"在翻译页打开"（`App.tsx` 已有 `draftText` 通道，复用即可）。

### P2-13 WebView 未设置 CSP

- 证据：`src-tauri/tauri.conf.json` 无 `app.security.csp`，Tauri 默认不注入策略。
- 建议：加 `default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'`（预览图是 `data:image/jpeg`，必须放行 `data:`）。本地应用收益有限但成本极低，属于纵深防御。

### P0-0 工程前置：git worktree 链接指向改名前的路径

- 证据：`.worktrees/quick-translate/.git` 内容为 `gitdir: D:/Codec_project/test_translate/.git/worktrees/quick-translate`，`git worktree list` 显示该路径 `prunable`；仓库已改名为 `Codex_project`。
- 影响：worktree 内所有 git 命令报 `fatal: not a git repository`，无法提交、无法看 diff、无法回滚。任何后续改动都没法留存。
- 建议：把该行改成 `gitdir: D:/Codex_project/test_translate/.git/worktrees/quick-translate`，再在 worktree 里跑 `git status` 确认干净、`git log -1` 确认 HEAD 仍是 `031e135`。只改一个文本文件，不涉及代码。

## 3. 建议的实施顺序

| 批次 | 内容 | 为什么这个顺序 |
| --- | --- | --- |
| 批次 0 | P0-0 修 git + 记录 `npm run check` 基线 | 不修 git，后面所有改动都无法提交和对比 |
| 批次 1 | P0-1 关窗退出、P0-2 历史 provider 回退、P0-3 字节计数、P0-4 删除服务 | 都是"用户一定会撞到"的功能缺陷，改动小、可测 |
| 批次 2 | P1-5 registry 缓存、P1-6 单次读设置、P1-7 预览降采样、P1-8 并行建窗 | 先打点拿数据，再按数据决定做到哪一步，避免为优化而优化 |
| 批次 3 | P2-9 到 P2-13 | 纯打磨，可随时停 |

每批结束后跑完整 `npm run check`（TypeScript + 26 个 UI 测试 + rustfmt + cargo test + 生产构建），并在 `docs/manual/windows-acceptance.md` 补对应的人工验收步骤。批次 2 完成后重新打一次 NSIS 包，确认产物大小与启动延迟没有回退。

## 4. 需要你先决策的三点

1. **历史"重新翻译"是否应该写新记录**：现在每次重译都会新增一行（`commands/translation.rs:244`），历史会重复膨胀。选项：保持现状 / 更新原记录 / 重译不写历史。
2. **预热浮层（P1-8 方案 b）做不做**：收益最大但会碰会话生命周期，风险比并行建窗高。建议先只做 (a)，实测不够再上 (b)。
3. **自动重试**：网络/超时自动重试一次要不要加？它改变"用户发起几次请求"的语义，加的话需要在 README 里说明。

## 5. 本次明确不做

- 不新增翻译服务适配器、不做术语表、不做批量文件翻译、不做剪贴板/划词监听（这些是设计文档 §8 的新功能，另开方案）。
- 不重构模块边界，不改 storage schema（除非 P1-6 需要加列，届时提供迁移脚本）。
- 不动安装器与 D 盘数据目录约定。

## 6. 实施记录（2026-11-08）

决策已确认：历史「重新翻译」更新原记录；浮层只做并行建窗、不做预热；网络/超时自动重试一次。

| 项目 | 状态 | 落点 |
| --- | --- | --- |
| P0-0 git worktree 链接 | 已完成 | `.worktrees/quick-translate/.git` 与 `.git/worktrees/quick-translate/gitdir` 均指向 `Codex_project`；`git worktree list` 不再 `prunable` |
| P0-1 关窗即退出 | 已完成 | `lib.rs` 新增 `on_window_event`：`main` 的 `CloseRequested` 走 `prevent_close` + `hide`；托盘「退出」仍调用 `app.exit(0)` |
| P0-2 历史重译 | 已完成 | `HistoryRepository::get/update`、`AppServices::retranslate_history_entry`、`retranslate_history_entry` 命令；`translate_with` 对已消失的 provider 回退到当前默认并如实返回实际 provider |
| P0-3 字节计数 | 已完成 | `ProviderMetadata.limit_bytes` → `ProviderOptionDto.limitBytes`；前端 `utf8Bytes` 计数、超限禁用按钮、提示带实际字节数 |
| P0-4 删除服务 | 已完成 | `AppServices::delete_provider_config` + 命令 + 设置页二次确认；先删凭据再删配置，删当前默认时自动改选 |
| P1-5 注册表缓存 | 已完成（未打点） | `provider_fingerprint` + `Mutex<Option<CachedRegistry>>`；配置序列化变化即失效，凭据按请求读取故无需失效 |
| P1-6 单次读设置 | 已完成 | `SettingsRepository::history_enabled()` 默认实现 + SQLite 侧 `json_extract` 覆盖 |
| P1-7 预览降采样 | 已完成 | 最长边 2560、`Triangle` 重采样；BGRA→RGBA 与 RGB 打包合并为一次遍历；OCR 仍用全分辨率 |
| P1-8 并行建窗 | 已完成（未实测） | 每屏 `spawn` 建窗后汇总；失败统一清理并恢复主窗；建窗后复查 `is_current` / `is_finished` |
| P2-9 重试与等待提示 | 已完成 | `ProviderError::is_transient` + 300 ms 后重试一次；`AppErrorDto.retryAfterSeconds` 透出 `Retry-After` |
| 追加：并发打开 SQLite | 已完成 | 测试暴露 `SQLITE_READONLY`（Windows 上两个连接竞争建库）；`storage/mod.rs` 加进程内 `OPEN_LOCK` 并对 BUSY/LOCKED/READONLY 重试 |

验证：`npm run check` 通过 —— TypeScript 无错误、30 个 UI 测试、rustfmt、73 个 Rust 测试、2 个编译失败 doctest、生产构建成功。

未做（原批次 3 打磨项，留待确认）：P2-10 设置页未保存更改提示、P2-11 快捷键/语言互换、P2-12 历史搜索与「在翻译页打开」、P2-13 CSP。

两处需要在真机确认收益或行为：P1-7/P1-8 的实际耗时改善没有在真机上测量；P0-1 的关窗行为需要 `tauri dev` 或安装包实测（自动测试无法覆盖桌面壳生命周期）。对应步骤已写入 `docs/manual/windows-acceptance.md`。
