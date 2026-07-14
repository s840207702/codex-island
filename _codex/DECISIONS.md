# 重要决策

- 2026-07-13：OpenAI 暂时取消 5 小时额度期间，仅展示周额度；从非空额度窗口中选择周期最长者，旧双窗口实现以注释保留。
- 2026-07-13：macOS 使用 Accessory 激活策略，保留动态岛与托盘，不占 Dock；详情窗口纳入 Tauri capability。
- 2026-07-13：macOS 本地包采用 ad-hoc 签名并提供 Universal 2 构建；公开分发需 Developer ID Application 签名和公证。
- 2026-07-13：macOS 动态岛不使用浅色内描边或浅色 border，避免透明窗口底部出现白色光边；保留纯暗色背景与柔和暗阴影。
- 2026-07-13：macOS 配置必须开启 `app.macOSPrivateApi`，否则 Tauri transparent window 在 WebView 周围退化为白色矩形。
- 2026-07-13：展开时主窗口会扩展到 520px 以承载详情定位，macOS 鼠标命中判断只认实际可见的 236px 胶囊，并用窗口失焦作为自动收回兜底。
- 2026-07-13：macOS 动态岛窗口使用 `NSStatusWindowLevel`（而非 Tauri 默认 Floating level）保证详情层级，但常驻位置取当前显示器 work area 顶部，避开物理刘海；非 macOS 仍使用普通 always-on-top。
- 2026-07-13：透明窗口的字体、去浅色边缘和 active 阴影修复只挂在 `platform-macos`，Windows 保留原有 `overrides.css` 视觉规则。
- 2026-07-14：macOS 沉浸模式复用共享前端状态机，但原生触发条件改为“前台应用的正常窗口完整覆盖实际 236×46 胶囊区域”；普通最大化和手动覆盖均可触发，窗口恢复、移开或仅部分相交时退出。Quartz 前台筛选会忽略尺寸不足以容纳胶囊的小型辅助浮层。Windows 保留原有全屏判定。
- 2026-07-13：macOS 构建脚本统一覆盖并启动 `/Applications/Codex Island.app`，验证时要求仅有一个 `codex-island` 进程，避免构建包与安装包双实例重叠导致悬停展开后看似无法折叠。
- 2026-07-14：公开仓库的产品截图必须来自 `/Applications/Codex Island.app` 真实运行窗口；`v1.0.3` Release 提供 Universal 2 DMG，并明确标注当前为 ad-hoc 签名、尚未公证。
- 2026-07-14：跨平台打包工作流直接维护在本仓库中；手动运行只生成 Actions Artifact，推送 `v*` 标签时由 Windows、macOS、Linux 原生 Runner 构建并在全部成功后发布同一个 Release。独立工作流模板仓库保持私有，不作为本公开项目的运行时依赖。
- 2026-07-14：额度刷新不再依赖 React/WebView 的 `setTimeout`。Windows 与 macOS 共用 Rust 原生后台轮询，成功后 60 秒刷新；失败时从 30 秒开始指数退避且最多 30 分钟，后台按系统时间检查以便睡眠唤醒后及时补刷新。详情面板默认读取共享缓存，但用户点击刷新时必须强制访问服务端。
