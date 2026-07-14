# 项目地图

- `src/main.tsx`：额度界面、状态与多语言文案。
- `src/overrides.css`：动态岛现行界面覆盖样式。
- `src-tauri/src/lib.rs`：Codex 登录态读取、OpenAI 额度接口映射与桌面窗口逻辑。
- `src-tauri/tauri.macos.conf.json`：macOS 图标、最低系统版本与本地签名配置。
- `script/build_and_run.sh`：macOS 停止旧进程、构建、启动与验证入口。
- `.codex/environments/environment.toml`：Codex 桌面端 Run 按钮配置。
