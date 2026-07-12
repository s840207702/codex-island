use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{menu::{Menu, MenuItem}, tray::TrayIconBuilder, LogicalPosition, LogicalSize, Manager, Position, Size, WebviewWindow};

#[derive(Serialize)] struct Window { used_percent: f64, remaining_percent: f64, reset_after_seconds: i64, reset_at: Option<Value> }
#[derive(Serialize)] struct Usage { primary: Window, secondary: Window, plan_type: String, plan_multiplier: Option<String>, reset_credits: Option<i64>, reset_credit_expires_at: Option<Value>, credit_balance: Option<f64>, has_credits: bool, fetched_at: String }
#[derive(Deserialize)] struct Auth { tokens: Tokens }
#[derive(Deserialize)] struct Tokens { access_token: String, account_id: Option<String> }
#[derive(Serialize, Deserialize)] struct SavedWindowPosition { x: f64, y: f64 }
#[derive(Serialize)] struct ImmersiveState { active: bool }

fn position_file() -> Option<std::path::PathBuf> { dirs::config_dir().map(|dir| dir.join("codex-island").join("window-position.json")) }

fn parse_window(v: &Value) -> Result<Window, String> {
    let used = v.get("used_percent").and_then(Value::as_f64).unwrap_or(0.0).clamp(0.0, 100.0);
    Ok(Window { used_percent: used, remaining_percent: 100.0 - used, reset_after_seconds: v.get("reset_after_seconds").and_then(Value::as_i64).unwrap_or(0), reset_at: v.get("reset_at").cloned() })
}
fn find_expiry(value: &Value) -> Option<Value> {
    match value {
        Value::Object(map) => {
            for key in ["expires_at", "expiresAt", "expiration_time", "expirationTime", "expires"] { if let Some(found) = map.get(key) { return Some(found.clone()); } }
            for key in ["credits", "reset_credits", "items", "available", "grants"] { if let Some(found) = map.get(key).and_then(find_expiry) { return Some(found); } }
            None
        },
        Value::Array(items) => items.iter().find_map(find_expiry),
        _ => None,
    }
}
fn center_window(window: &WebviewWindow, width: f64, height: f64) -> Result<(), String> {
    let monitor = window.current_monitor().map_err(|e| e.to_string())?.ok_or("未找到显示器")?;
    let scale = monitor.scale_factor(); let size = monitor.size().to_logical::<f64>(scale); let position = monitor.position().to_logical::<f64>(scale);
    window.set_position(Position::Logical(LogicalPosition::new(position.x + (size.width - width) / 2.0, position.y))).map_err(|e| e.to_string())?;
    window.set_size(Size::Logical(LogicalSize::new(width, height))).map_err(|e| e.to_string())
}
fn restore_window_position(window: &WebviewWindow, width: f64, height: f64) -> Result<(), String> {
    let restored = position_file().and_then(|path| std::fs::read_to_string(path).ok()).and_then(|raw| serde_json::from_str::<SavedWindowPosition>(&raw).ok());
    if let Some(saved) = restored {
        // Migrate the legacy initial offset (6px native + 10px web padding) to the true top edge.
        let y = if saved.y <= 16.0 { 0.0 } else { saved.y };
        window.set_position(Position::Logical(LogicalPosition::new(saved.x, y))).map_err(|e| e.to_string())?;
        window.set_size(Size::Logical(LogicalSize::new(width, height))).map_err(|e| e.to_string())
    } else { center_window(window, width, height) }
}
#[tauri::command]
async fn fetch_usage() -> Result<Usage, String> {
    let path = dirs::home_dir().ok_or("无法定位用户目录")?.join(".codex").join("auth.json");
    let auth: Auth = serde_json::from_str(&std::fs::read_to_string(path).map_err(|_| "未找到 Codex 登录态，请先登录 Codex")?).map_err(|_| "Codex 登录态格式无效")?;
    let client = reqwest::Client::new();
    let mut request = client.get("https://chatgpt.com/backend-api/wham/usage").bearer_auth(&auth.tokens.access_token).header("User-Agent", "CodexIsland/0.1 (local-only)");
    if let Some(id) = auth.tokens.account_id { request = request.header("ChatGPT-Account-ID", id); }
    let body: Value = request.send().await.map_err(|_| "无法连接 OpenAI 额度接口")?.error_for_status().map_err(|e| format!("OpenAI 额度接口错误：{}", e.status().map(|x| x.as_u16()).unwrap_or(0)))?.json().await.map_err(|_| "OpenAI 返回的额度数据无法解析")?;
    let limit = body.get("rate_limit").ok_or("OpenAI 未返回额度窗口")?;
    let credits = body.get("credits").unwrap_or(&Value::Null);
    let reset = body.get("rate_limit_reset_credits").unwrap_or(&Value::Null);
    let mut reset_request = client.get("https://chatgpt.com/backend-api/wham/rate-limit-reset-credits").bearer_auth(&auth.tokens.access_token).header("User-Agent", "CodexIsland/0.1 (local-only)");
    if let Some(id) = body.get("account_id").and_then(Value::as_str) { reset_request = reset_request.header("ChatGPT-Account-ID", id); }
    let reset_detail = match reset_request.send().await { Ok(response) => match response.error_for_status() { Ok(response) => response.json::<Value>().await.ok(), Err(_) => None }, Err(_) => None };
    let reset_credit_expires_at = reset_detail.as_ref().and_then(find_expiry).or_else(|| find_expiry(reset));
    Ok(Usage { primary: parse_window(limit.get("primary_window").ok_or("缺少短期额度")?)?, secondary: parse_window(limit.get("secondary_window").ok_or("缺少周额度")?)?, plan_type: body.get("plan_type").and_then(Value::as_str).unwrap_or("unknown").to_owned(), plan_multiplier: body.get("promo").and_then(|p| p.get("multiplier").or_else(|| p.get("rate_limit_multiplier"))).and_then(Value::as_str).map(str::to_owned), reset_credits: ["available_count", "availableCount", "remaining", "count"].iter().find_map(|key| reset.get(*key).and_then(Value::as_i64)), reset_credit_expires_at, credit_balance: credits.get("balance").and_then(Value::as_f64), has_credits: credits.get("has_credits").and_then(Value::as_bool).unwrap_or(false), fetched_at: chrono_like_now() })
}
fn chrono_like_now() -> String { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs().to_string() }
#[tauri::command] fn set_expanded(window: WebviewWindow, expanded: bool, immersive: bool) -> Result<(), String> {
    // Resizing leaves the user-selected window position unchanged.
    // Immersive mode only changes the inner visual capsule. Keeping the native window
    // at the collapsed size preserves the island's screen anchor and avoids a left shift.
    let (width, height) = if expanded && !immersive { (540, 420) } else { (540, 64) };
    // The React layout uses CSS pixels. Logical sizing keeps that layout stable
    // at 100%, 125%, 150%, and 200% Windows DPI scaling.
    window.set_always_on_top(true).map_err(|e| e.to_string())?;
    window.set_size(Size::Logical(LogicalSize::new(width as f64, height as f64))).map_err(|e| e.to_string())?;
    Ok(())
}
#[cfg(target_os = "windows")]
#[tauri::command] fn get_immersive_state(_window: WebviewWindow) -> Result<ImmersiveState, String> {
    use windows::Win32::{Foundation::RECT, Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITOR_DEFAULTTONEAREST, MONITORINFO}, System::Threading::GetCurrentProcessId, UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId}};
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.0.is_null() { return Ok(ImmersiveState { active: false }); }
        let mut foreground_pid = 0;
        GetWindowThreadProcessId(foreground, Some(&mut foreground_pid));
        if foreground_pid == GetCurrentProcessId() { return Ok(ImmersiveState { active: false }); }
        let mut foreground_rect = RECT::default();
        if GetWindowRect(foreground, &mut foreground_rect).is_err() { return Ok(ImmersiveState { active: false }); }
        let monitor = MonitorFromWindow(foreground, MONITOR_DEFAULTTONEAREST);
        let mut monitor_info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if !GetMonitorInfoW(monitor, &mut monitor_info).as_bool() { return Ok(ImmersiveState { active: false }); }
        let screen = monitor_info.rcMonitor;
        // Ordinary maximized windows are deliberately excluded: only a true full-screen
        // foreground surface should make the island reduce itself.
        let tolerance = 2;
        Ok(ImmersiveState { active: foreground_rect.left <= screen.left + tolerance && foreground_rect.top <= screen.top + tolerance && foreground_rect.right >= screen.right - tolerance && foreground_rect.bottom >= screen.bottom - tolerance })
    }
}
#[cfg(not(target_os = "windows"))]
#[tauri::command] fn get_immersive_state() -> Result<ImmersiveState, String> { Ok(ImmersiveState { active: false }) }
#[tauri::command] fn save_window_position(window: WebviewWindow) -> Result<(), String> {
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let position = window.outer_position().map_err(|e| e.to_string())?.to_logical::<f64>(scale);
    let path = position_file().ok_or("无法定位应用设置目录")?;
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
    std::fs::write(path, serde_json::to_string(&SavedWindowPosition { x: position.x, y: position.y }).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
#[tauri::command] fn start_window_drag(window: WebviewWindow) -> Result<(), String> { window.start_dragging().map_err(|e| e.to_string()) }
#[tauri::command] fn exit_app(app: tauri::AppHandle) { app.exit(0); }
fn show_main_window(app: &tauri::AppHandle) { if let Some(window) = app.get_webview_window("main") { let _ = window.show(); let _ = window.set_focus(); } }
pub fn run() { tauri::Builder::default().plugin(tauri_plugin_opener::init()).setup(|app| {
    if let Some(window) = app.get_webview_window("main") { let _ = window.set_always_on_top(true); let _ = restore_window_position(&window, 540.0, 64.0); }
    let show = MenuItem::with_id(app, "show", "显示 Codex Island", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "隐藏 Codex Island", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &hide, &quit])?;
    let mut tray = TrayIconBuilder::with_id("codex-island-tray").tooltip("Codex Island").menu(&menu);
    if let Some(icon) = app.default_window_icon() { tray = tray.icon(icon.clone()); }
    tray.on_menu_event(|app, event| match event.id.as_ref() { "show" => show_main_window(app), "hide" => { if let Some(window) = app.get_webview_window("main") { let _ = window.hide(); } }, "quit" => app.exit(0), _ => {} }).build(app)?;
    Ok(())
}).invoke_handler(tauri::generate_handler![fetch_usage, set_expanded, get_immersive_state, save_window_position, start_window_drag, exit_app]).run(tauri::generate_context!()).expect("error while running Codex Island"); }
