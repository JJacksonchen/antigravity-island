# Antigravity Island

<p align="center">
  <strong>A lightweight, pure-Rust Dynamic Island &amp; desktop quota widget for Google Antigravity on macOS &amp; Windows — engineered for instant at-a-glance readability, local loopback sync, and hot-reloadable custom skins.</strong><br/>
  <strong>纯 Rust 编写的高性能 Google Antigravity 双端（macOS / Windows）灵动岛与桌面额度悬浮插件：抬眼即知剩余额度，毫秒级本地实时同步，支持热重载自定义皮肤。</strong>
</p>

<p align="center">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-1.91%2B-orange?logo=rust"/>
  <img alt="Platform" src="https://img.shields.io/badge/Platform-macOS%20%7C%20Windows%2010%2F11%20%7C%20Linux-blue"/>
  <img alt="Binary Size" src="https://img.shields.io/badge/Release%20Binary-7.3%20MB-brightgreen"/>
  <img alt="License" src="https://img.shields.io/badge/License-MIT-green"/>
</p>

<p align="center">
  <img src="assets/cross-platform-glance.png" alt="Antigravity Island At-a-Glance Dual-Platform Design (macOS & Windows)" width="100%"/>
</p>

---

## One-Click Install & Deploy / 双端一键部署安装

### macOS（Apple Silicon / Intel）

自动下载 GitHub Releases 最新预编译二进制（无须编译，若不可用自动回退 `cargo` 源码构建），安装 CLI + `Antigravity Island.app`（`LSUIElement = true`，不占 Dock 栏图标）并立即启动：

```bash
# 一键安装并立即启动
curl -fsSL https://raw.githubusercontent.com/JJacksonchen/antigravity-island/main/scripts/install.sh | bash

# 一键安装 + 配置 macOS 开机自启动 (LaunchAgent)
curl -fsSL https://raw.githubusercontent.com/JJacksonchen/antigravity-island/main/scripts/install.sh | bash -s -- --autostart

# 一键卸载（加 --purge-config 同时清理配置文件）
curl -fsSL https://raw.githubusercontent.com/JJacksonchen/antigravity-island/main/scripts/uninstall.sh | bash
```

### Windows 10 / 11（PowerShell）

自动下载预编译 `antigravity-island-windows-x86_64.zip`（无须 Rust 环境，或传 `-FromSource` 本地编译），安装至 `%LOCALAPPDATA%\AntigravityIsland`、自动加入用户 `PATH`、创建开始菜单快捷方式并无黑框后台启动：

```powershell
# 一键安装并立即启动
irm https://raw.githubusercontent.com/JJacksonchen/antigravity-island/main/scripts/install.ps1 | iex

# 一键安装 + 配置 Windows 开机自启动 (HKCU\...\Run)
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/JJacksonchen/antigravity-island/main/scripts/install.ps1))) -Autostart

# 一键卸载（加 -PurgeConfig 同时清理配置目录）
irm https://raw.githubusercontent.com/JJacksonchen/antigravity-island/main/scripts/uninstall.ps1 | iex
```

> **Tip**：尚未打开 Antigravity 时，可运行 `antigravity-island --demo` 立即体验实时动画额度流与全部皮肤效果。

---

## At-a-Glance Dual-Platform Design / 双端「看一眼就知道额度」设计

传统查看 **Google Antigravity** 配额需要切换窗口、打开 **Settings → Models** 或敲 `/usage`。**Antigravity Island** 在 macOS 和 Windows 上均坚持 **零点击、抬眼即知** 的信息层级：

- **双行一眼看清折叠胶囊（Glance Pill，默认）**：无需悬停即可直接读取每个模型池的 **同心双环（外环 5h + 内环周额度）**，第一行清晰标注 **`5h` 徽标 + 粗体剩余百分比 + `⏱` 5h 刷新倒计时**，第二行独立展示 **`周 / 7d` 剩余百分比 + 迷你周进度条**；胶囊底部自带全局 **环境健康度光轨**，5h 额度、周额度与倒计时彻底分离、零混淆。
- **双端自由拖拽 + 一键隐藏**：macOS 与 Windows 无论在 `Island` 还是 `Widget` 形态下，均支持 **直接拖拽折叠胶囊或展开标题栏** 放置于任意屏幕位置（自动记忆坐标，双击标题栏或点 `居中置顶` 一键复位）；支持 **右键胶囊 / 点击眼睛图标 / 托盘菜单** 一键隐藏与恢复。
- **macOS 零遮挡、零重复状态栏联动**：默认悬浮在 macOS 菜单栏下方清爽居中（绝不遮挡菜单栏）；灵动岛可见时，顶部菜单栏自动切换为极简图标（避免与灵动岛数值重复）；当灵动岛隐藏时，菜单栏自动显示实时额度文字（`G 65%/64% · C 100%`），亦可在设置中开启「始终显示状态栏文字」。
- **Windows 专属双保险**：顶部置顶透明灵动岛（`WS_EX_TOOLWINDOW` + `skip_taskbar`，不占任务栏与 `Alt+Tab`、无 1px 白边、无黑框闪烁）+ **系统托盘动态彩色进度环图标**（左键托盘图标直接切换显示/隐藏，右键打开完整菜单，悬停显示多行配额与重置明细）。
- **极简模式随时切换**：在面板设置或托盘菜单中可随时切换 `Glance（双行一眼看清）` 与 `Compact（极简胶囊）`。

<p align="center">
  <img src="assets/hero-showcase.png" alt="Antigravity Island Expanded Card Showcase" width="100%"/>
</p>

---

## 6 Built-in Skins / 内置皮肤与仪表展示

<p align="center">
  <img src="assets/skins-gallery.png" alt="Antigravity Island 6 Built-in Skins Gallery" width="100%"/>
</p>

| Skin ID | Name | Gauge Style | Aesthetic & Highlights |
| :--- | :--- | :--- | :--- |
| `obsidian` | **Obsidian Island** | `ring` | 经典哑光黑 Apple 灵动岛风格，清晰 SVG 圆环与低占用呼吸灯 |
| `frosted-glass` | **Liquid Glass** | `bar` | 毛玻璃半透明磨砂质感，带高光边缘与渐变进度条 |
| `cyber-neon` | **Cyber Neon** | `segments` | 赛博霓虹 HUD，青/品红 12 格发光激光分段条 |
| `matcha-paper` | **Matcha Paper** | `bar` | 日间暖调和纸手账风格，抹茶绿与赤陶橙护眼配色 |
| `solar-aurora` | **Solar Aurora** | `wave` | 极光渐变背景搭配动态 SVG 波浪液态能量条 |
| `retro-pixel` | **Retro CRT** | `segments` | 复古荧光绿等宽终端扫描线风格，极客硬件仪表感 |

---

## Operation Guide / 详细操作示意图

<p align="center">
  <img src="assets/operation-guide.png" alt="Antigravity Island Detailed Operation Guide" width="100%"/>
</p>

| Operation / Button | Action / Behavior |
| :--- | :--- |
| **Hover Pill / 悬停胶囊** | `Island` / `Notch Hug` 模式下悬停 180ms 丝滑展开完整面板，移出 260ms 自动收起 |
| **Drag Pill or Header / 自由拖拽** | macOS 与 Windows 的 `Island` / `Widget` 形态均支持按住胶囊或标题栏任意拖动并记忆位置，双击标题栏恢复顶部居中 |
| **Right-Click Pill or `👁` Hide / 一键隐藏** | 右键折叠胶囊或点击展开卡片右上角/底部隐藏按钮即可隐藏；Windows 左键托盘图标或 macOS/Win 托盘菜单可随时恢复 |
| **`📌` Pin / 图钉** | 锁定保持展开状态，悬停移出不收起；再次点击或点 `收起` 折叠 |
| **`↻` Refresh / 刷新** | 立即向本地 Antigravity 后台发起一次最新配额查询 |
| **`◫` Mode / 形态切换** | 一键切换 **Dynamic Island**（悬停展开·可拖拽）、**Notch Hug**（macOS 刘海融合）与 **Desktop Widget**（点击展开·可拖拽） |
| **`🎨` Skin Studio / 皮肤工坊** | 实时切换 6 套内置皮肤或自定义仪表样式、强调色、圆角、透明度、呼吸光晕，一键保存为 `.toml` |
| **`⚙` Settings / 偏好设置** | 切换形态、胶囊信息密度（`Glance / Compact`）、重置居中置顶、macOS 状态栏文字开关、刷新间隔、语言与低额度提醒 |
| **Tray / Menu Bar** | macOS 隐藏时自动接管显示实时额度，显示时自动切回极简图标；Windows 托盘彩色进度环左键显隐、右键菜单 |

---

## Skin Studio & Real-Time Sync Architecture / 皮肤热重载与实时同步原理

<p align="center">
  <img src="assets/skin-studio-sync.png" alt="Skin Studio and Real-Time Local Sync Architecture" width="100%"/>
</p>

### Local Sync Pipeline / 双端本地同步链路

```mermaid
flowchart LR
    A["Antigravity 2.34+ App (macOS & Win)<br/>~/.gemini/antigravity/daemon/hub_*.json"] --> D["Discovery & Cache"]
    B["Antigravity IDE / Legacy App<br/>ps / Win32_Process + lsof / netstat"] --> D
    C["agy CLI<br/>statusLine / agy -p /usage"] --> D
    D --> E["Loopback Connect-RPC Client<br/>127.0.0.1 + .no_proxy()"]
    F["FS Activity Watcher<br/>~/.gemini/antigravity/*"] -->|"2.2s Debounce"| E
    G["Reset-Zero Timer"] -->|"Auto Refresh"| E
    E --> H["QuotaSyncEngine (Tokio)"]
    H --> I["Glance Pill + Expanded Card + OS Tray"]
```

1. **双端零配置自动发现**：优先读取 Antigravity 2.34+ 写入的 `~/.gemini/antigravity/daemon/hub_*.json`（Windows 为 `%USERPROFILE%\.gemini\antigravity\daemon\hub_*.json`），校验进程 PID 存活后直连本地端口；同时兼容传统 `--csrf_token` 进程发现（macOS `ps` + `lsof`，Windows `Win32_Process` + `netstat`，全程 `CREATE_NO_WINDOW` 静默无黑框）。
2. **三段式优雅回退**：`RetrieveUserQuotaSummary`（获取完整 5h / Weekly 分组池）+ `GetUserStatus`（补充账号 Tier / AI Credits）→ `GetUserStatus` 模型配额 → `GetCommandModelConfigs` → CLI `statusLine` / `agy -p /usage`。
3. **绕过系统代理**：回环客户端内置 `.no_proxy()`，即使开启 Clash / Surge / 系统代理也不会拦截 `127.0.0.1` RPC。
4. **轻量高性能**：纯 Rust 单文件 Release 二进制约 **7.3 MB**，系统原生 WebView（macOS WKWebView / Windows WebView2）常驻内存约 **67–77 MB RSS**，空闲 **0.0% CPU**。

---

## CLI Usage & `agy statusLine` Integration / 命令行用法

```text
antigravity-island [OPTIONS] [COMMAND]

Options:
  --demo                 以模拟实时额度流启动 GUI（无需运行 Antigravity）
  --skin <ID>            指定启动皮肤 ID（如 cyber-neon）
  --mode <MODE>          指定展示形态：island | notch-hug | widget
  --probe                执行一次实时额度探测并输出到终端后退出
  --json                 配合 --probe 输出完整 JSON 快照
  --list-skins           列出所有内置及自定义皮肤

Commands:
  ingest-statusline      从 stdin 读取 Antigravity CLI statusLine JSON 并推送给灵动岛
```

### Example: Wire into `agy` CLI `statusLine`

在 `~/.gemini/antigravity-cli/settings.json` 中添加：

```json
{
  "statusLine": {
    "type": "command",
    "command": "antigravity-island ingest-statusline"
  }
}
```

---

## Authoring Custom Skins / 手写自定义皮肤

自定义皮肤位于 `~/.config/antigravity-island/skins/*.toml`（Windows 位于 `%USERPROFILE%\.config\antigravity-island\skins\*.toml`），编辑保存后毫秒级热重载，无需重启。参考模板：[`skins/cyberpunk-neon-sample.toml`](skins/cyberpunk-neon-sample.toml)。

---

## Build from Source & Testing / 源码构建与自测

```bash
git clone https://github.com/JJacksonchen/antigravity-island.git
cd antigravity-island

# Format, lint & run all 19 unit/integration tests
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test

# macOS: package .app bundle locally
./scripts/bundle-macos.sh
open "Antigravity Island.app"
```

### Privacy & Security / 隐私与安全声明

- 所有本地请求严格校验目标主机必须为 `127.0.0.1`，绝不向外部域名传输本机 CSRF Token。
- 不读取、不存储 Google OAuth Refresh Token 或任何云端凭证，零遥测上报。

---

## License

[MIT](LICENSE)
