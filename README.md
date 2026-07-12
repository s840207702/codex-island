# Codex Island

一个可扩展的 Windows 顶部动态岛。当前读取本机已有的 Codex 登录态，显示 OpenAI 服务端返回的套餐、5 小时额度、周额度与重置时间。

## 特性

- 顶部动态岛：鼠标悬停展开；支持锁定常驻或自动收起。
- 两种视图：**概览** 同时呈现两个额度环；**专注** 突出当前 5 小时额度。
- 本地优先：令牌仅在 Rust 后端内存中用于请求 `chatgpt.com`，不写入前端、本地配置或第三方服务。

## 开发

```powershell
npm install
npm run tauri dev
```

## 构建

```powershell
npm run tauri build
```

需要先在本机登录 Codex，使 `~/.codex/auth.json` 存在。第三方工具不会绕过 OpenAI 的限流；若服务端额度自身存在同步延迟，界面会展示服务端当时返回的状态。
