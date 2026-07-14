# macOS 构建说明

Codex Island 的主界面、额度读取、托盘菜单与双窗口结构可直接运行在 macOS。macOS 还使用 Core Graphics 读取全局鼠标位置，保证悬停胶囊与详情面板之间的保持逻辑和 Windows 一致。

## 环境

- macOS 11 或更高版本
- Xcode Command Line Tools：`xcode-select --install`
- Node.js 20+
- Rust stable：`rustup default stable`

## 构建

```bash
pnpm install --frozen-lockfile
pnpm tauri:mac
```

输出目录为 `src-tauri/target/release/bundle/macos/` 和 `src-tauri/target/release/bundle/dmg/`。

如需同时支持 Apple Silicon 与 Intel Mac，请先安装两个 Rust target，再构建 Universal 2 安装包：

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
pnpm tauri:mac:universal
```

`src-tauri/tauri.macos.conf.json` 会在 macOS 自动与主 Tauri 配置合并，只生成 `.app` 和 `.dmg`，不会影响 Windows 的 NSIS 构建。

项目内统一使用 `./script/build_and_run.sh --verify` 完成 Universal 2 构建、覆盖安装到 `/Applications/Codex Island.app`、启动和单实例校验，避免构建目录与正式安装目录同时运行两个应用实例。

## 发布前

本地构建默认使用 ad-hoc 签名，保证 Apple Silicon 上的应用包结构与资源签名完整。公开分发前仍需替换为 Apple Developer ID Application 签名并完成公证，否则其他 Mac 仍可能要求用户在“隐私与安全性”中手动放行。

macOS 沉浸模式按前台窗口是否完整覆盖常驻胶囊区域触发；窗口最大化或手动移动到胶囊后方并完整覆盖时都会进入沉浸，恢复、移开或只覆盖一部分时回到常规胶囊。

macOS 运行时使用 Accessory 激活策略：应用保留顶部动态岛和状态栏托盘，但不额外占用 Dock 图标。

透明窗口依赖 Tauri 的 `app.macOSPrivateApi`（仅在 macOS 配置中打开），否则 WebView 会以白色矩形填充透明区域。

macOS WebView 的字体与透明窗口去边缘修复通过 `platform-macos` 作用域加载；Windows 继续使用原有 `overrides.css` 视觉规则。

沉浸模式与 Windows 共用前端状态机，但原生判定按平台隔离：macOS 通过 Quartz 读取前台应用的可见正常窗口，判断其是否完整覆盖实际 236×46 胶囊；小型辅助浮层、光标层和透明定位画布不会参与触发。Windows 继续使用原有全屏窗口判定。

展开详情时主窗口会保留透明定位画布，但自动收回只按可见胶囊判断；切换到其他应用失焦时也会触发收回。

动态岛主窗口和详情窗口在 macOS 使用 `NSStatusWindowLevel`，保证详情层级不被普通窗口盖住；常驻胶囊则取显示器 work area 顶部，避开物理刘海。Windows 等其他平台继续使用普通 always-on-top 层级。
