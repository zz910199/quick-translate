# Quick Translate

Windows 优先的个人快捷翻译工具：手动小窗口、截图框选和 Windows OCR、可编辑识别文本、服务切换、敏感模式、历史记录、剪贴板、全局快捷键和系统托盘。当前版本 `0.1.0`。

## 使用与安装

运行环境为 Windows 10/11 x64，需已安装 Microsoft Edge WebView2 Runtime；截图识别还需要对应的 Windows OCR 语言包。在 Windows 设置中为所需语言安装语言功能。缺少语言包会显示明确错误，不会偷偷换用另一种语言。安装包不会自动安装 WebView2。

构建后的 NSIS 安装包在 `src-tauri/target/release/bundle/nsis/`。安装前确认安装路径，并在路径页选择 D 盘（例如 `D:\QuickTranslate\App`）；不要使用静默安装。安装器按当前用户安装，正常情况下无需管理员权限。当前开发版本未签名，Windows 可能显示未知发布者提示。

应用专属数据固定为 `D:\QuickTranslate\Data`，包括 SQLite 和 WebView 数据；没有 D 盘时不会回退到 C 盘。API 密钥仅存入 Windows 凭据管理器（服务名 `quick-translate`），不在 SQLite 或 UI 中回显。Windows 本身管理的凭据、注册表和开始菜单项不属于可迁移的应用数据目录。

关闭主窗口只会隐藏窗口，进程、托盘图标和全局快捷键继续工作；要真正结束程序请用托盘菜单的「退出」。

## 翻译服务

首次运行默认使用 MyMemory，无需注册或密钥。原文会发送至 MyMemory，服务归属为 [Translated / MyMemory](https://mymemory.translated.net/)。当前适配器每次最多 500 UTF-8 字节；匿名额度参考值为每天 5,000 字符。输入框按字节显示用量（例如 `320 / 500 字节`），超限时翻译按钮直接禁用。免费服务的额度、可用性和政策可能变化，网络失败可保留原文重试。

网络错误和超时会在提示前自动重试一次；认证失败、额度不足和配置错误不会重试。额度不足时界面显示服务建议的等待秒数。

在“设置”中添加或修改服务，先保存服务配置，再单独保存密钥。密钥输入框保存后清空，只显示配置状态。不再使用的服务可以用「删除服务」移除，配置与已保存的密钥会一起清除；删除当前默认服务后会自动切换到下一个可用服务。

| 服务 | 配置 |
| --- | --- |
| DeepL | ID 必须为 `deepl`；Free endpoint 为 `https://api-free.deepl.com/v2/translate`，Pro 为 `https://api.deepl.com/v2/translate`；填入对应 API 密钥 |
| OpenAI 兼容服务 | 使用唯一 ID（例如 `openai-compatible`）、以 `/v1` 结尾的 HTTPS endpoint、服务商提供的模型名及 API 密钥 |
| 本地 OpenAI 兼容服务 | 例如 Ollama 的 `http://localhost:11434/v1`、已安装的本地模型名，勾选“本地服务”；不要求密钥，若服务器要求认证也可配置 |

同一协议可以配置多个独立 ID，随时切换。只有 `localhost`、`127.0.0.1`、`[::1]` 等回环地址允许标记本地。在线 endpoint 必须为 HTTPS；不支持 URL 内嵌账号、密钥、查询串或片段。

敏感模式只分派到已启用的本地服务；没有本地服务则报错，不会回退到远端。在线优先模式的自动 fallback 也只允许到本地服务。截图 OCR 在设备上完成，识别成功后可编辑文字；只有明确提交翻译时才调用翻译服务。本地服务是独立程序，其自身是否联网由用户管理。

默认截图快捷键为 `Ctrl+Shift+T`，可在设置中修改；冲突会给出错误，不会覆盖其他程序的快捷键。托盘支持打开翻译器、截图翻译和退出。

历史默认开启，保存在本机 SQLite 中，无自动过期；界面显示最近 100 条。关闭历史只停止新增，已有记录仍保留，可逐条删除或确认后清空全部历史。历史里的「重新翻译」会更新该条记录而不是新增一条；如果记录引用的服务已被删除，会回退到当前默认服务并在界面说明。卸载前按需要清理历史和服务密钥。

## 开发与验证

开发需要 Node.js 22+、npm、Rust stable MSVC 工具链、Visual Studio Build Tools 的 C++ 桌面构建工具与 Windows SDK、WebView2。前端为 React/TypeScript，桌面容器为 Tauri 2；领域、存储、provider 和 OCR 服务与 Windows API 实现分离，为后续跨平台适配保留边界，当前只验收 Windows。

本机工具和缓存统一放 D 盘。新 PowerShell 中可设置：

```powershell
$env:CARGO_HOME = 'D:\DevTools\Rust\cargo'
$env:RUSTUP_HOME = 'D:\DevTools\Rust\rustup'
$env:TEMP = 'D:\DevTools\Temp'
$env:TMP = $env:TEMP
$env:npm_config_cache = 'D:\DevTools\Cache\npm'
$env:LOCALAPPDATA = 'D:\DevTools\Cache\TauriLocal' # Vitest 等尊重此环境变量的工具缓存
$env:PATH = 'D:\DevTools\Rust\cargo\bin;D:\Program Files\nodejs;' + $env:PATH
npm ci
npm run tauri -- dev
```

```powershell
npm run check                  # TypeScript、UI 测试、rustfmt、Rust 测试、前端生产构建
npm run tauri -- build         # Release EXE + NSIS 当前用户安装包
# 等价 Windows 专用快捷命令：npm run package:windows
```

默认构建输出在项目 D 盘下的 `dist` 与 `src-tauri/target`。Tauri 的 `bundle.useLocalToolsDir` 已开启，NSIS 等打包工具缓存位于 `src-tauri/target/.tauri`，无需依赖 Windows 的 C 盘用户缓存路径。服务集成测试使用临时 SQLite、假凭据存储、假 OCR 和本机 HTTP mock server，不需要外部网络或真实凭据。

完整人工验收与当前限制见 [Windows acceptance](docs/manual/windows-acceptance.md)。多显示器和混合 DPI、操作系统快捷键冲突、真实凭据管理器及安装器运行需要相应 Windows 环境实测；自动测试不能替代这些检查。
