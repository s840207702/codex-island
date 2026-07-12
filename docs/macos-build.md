# macOS 构建说明

Codex Island 的主界面、额度读取、托盘菜单与双窗口结构可直接运行在 macOS。macOS 还使用 Core Graphics 读取全局鼠标位置，保证悬停胶囊与详情面板之间的保持逻辑和 Windows 一致。

## 环境

- macOS 11 或更高版本
- Xcode Command Line Tools：`xcode-select --install`
- Node.js 20+
- Rust stable：`rustup default stable`

## 构建

```bash
npm ci
npm run tauri:mac
```

输出目录为 `src-tauri/target/release/bundle/macos/` 和 `src-tauri/target/release/bundle/dmg/`。

`src-tauri/tauri.macos.conf.json` 会在 macOS 自动与主 Tauri 配置合并，只生成 `.app` 和 `.dmg`，不会影响 Windows 的 NSIS 构建。

## 发布前

公开分发前需要在 macOS 上为 `.app` / `.dmg` 配置 Apple Developer 签名和公证；未签名的开发构建可能会被 Gatekeeper 拦截。

当前 macOS 沉浸模式采取保守策略：只保留稳定的常规胶囊与悬停展开，避免把普通窗口误判为全屏而隐藏岛屿。
