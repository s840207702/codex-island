use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{menu::{Menu, MenuItem}, tray::TrayIconBuilder, LogicalPosition, LogicalSize, Manager, Position, Size, WebviewWindow};

#[derive(Serialize)] struct Window { used_percent: f64, remaining_percent: f64, reset_after_seconds: i64, reset_at: Option<Value> }
#[derive(Serialize)] struct Usage { primary: Window, secondary: Window, plan_type: String, plan_multiplier: Option<String>, reset_credits: Option<i64>, reset_credit_expires_at: Option<Value>, credit_balance: Option<f64>, has_credits: bool, fetched_at: String }
#[derive(Deserialize)] struct Auth { tokens: Tokens }
#[derive(Deserialize)] struct Tokens { access_token: String, account_id: Option<String> }
#[derive(Serialize, Deserialize)] struct SavedWindowPosition { x: f64, y: f64, #[serde(default)] user_moved: bool }
#[derive(Serialize)] struct ImmersiveState { active: bool }
#[cfg(target_os = "windows")]
struct ImmersiveScan { island: windows::Win32::Foundation::RECT, own_pid: u32, active: bool }
#[cfg(target_os = "windows")]
unsafe extern "system" fn scan_immersive_window(hwnd: windows::Win32::Foundation::HWND, data: windows::Win32::Foundation::LPARAM) -> windows::core::BOOL {
    use windows::{core::BOOL, Win32::{Foundation::RECT, Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITOR_DEFAULTTONEAREST, MONITORINFO}, UI::WindowsAndMessaging::{GetClassNameW, GetWindowLongW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, GWL_EXSTYLE, GWL_STYLE, WS_CAPTION, WS_EX_TOPMOST}}};
    let scan = &mut *(data.0 as *mut ImmersiveScan);
    if !IsWindowVisible(hwnd).as_bool() { return BOOL(1); }
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == scan.own_pid { return BOOL(1); }
    let mut title_buffer = [0u16; 256];
    let mut class_buffer = [0u16; 256];
    let title_len = GetWindowTextW(hwnd, &mut title_buffer).max(0) as usize;
    let class_len = GetClassNameW(hwnd, &mut class_buffer).max(0) as usize;
    let identity = format!("{} {}", String::from_utf16_lossy(&title_buffer[..title_len]), String::from_utf16_lossy(&class_buffer[..class_len])).to_ascii_lowercase();
    if ["progman", "workerw", "desktop", "fence", "textinputhost", "windows 输入体验"].iter().any(|term| identity.contains(term)) { return BOOL(1); }
    let mut rect = RECT::default();
    if GetWindowRect(hwnd, &mut rect).is_err() { return BOOL(1); }
    let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    if !GetMonitorInfoW(monitor, &mut info).as_bool() { return BOOL(1); }
    let screen = info.rcMonitor;
    let tolerance = 2;
    let fills_monitor = rect.left <= screen.left + tolerance && rect.top <= screen.top + tolerance && rect.right >= screen.right - tolerance && rect.bottom >= screen.bottom - tolerance;
    let covers_island = rect.left <= scan.island.left && rect.top <= scan.island.top && rect.right >= scan.island.right && rect.bottom >= scan.island.bottom;
    let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
    let extended_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
    let frameless = (style & WS_CAPTION.0) == 0;
    let topmost = (extended_style & WS_EX_TOPMOST.0) != 0;
    if (fills_monitor && (frameless || topmost)) || (topmost && covers_island) { scan.active = true; return BOOL(0); }
    BOOL(1)
}

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
    if let Some(saved) = restored.filter(|position| position.user_moved) {
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
#[tauri::command] fn set_expanded(window: WebviewWindow, expanded: bool, immersive: bool, content_width: Option<f64>, content_height: Option<f64>) -> Result<(), String> {
    // Resize tightly around the actual visible island while preserving its visual center.
    // Immersive mode only changes the inner visual capsule. Keeping the native window
    // at the collapsed size preserves the island's screen anchor and avoids a left shift.
    let (width, height) = if immersive {
        // The immersive window is click-through, so keep its full visual canvas intact.
        (520.0, 64.0)
    } else {
        let (fallback_width, fallback_height) = if expanded { (520.0, 397.0) } else { (236.0, 46.0) };
        (content_width.unwrap_or(fallback_width), content_height.unwrap_or(fallback_height))
    };
    // The React layout uses CSS pixels. Logical sizing keeps that layout stable
    // at 100%, 125%, 150%, and 200% Windows DPI scaling.
    window.set_always_on_top(true).map_err(|e| e.to_string())?;
    // Immersive mode is display-only: every pointer event goes to the app underneath.
    window.set_ignore_cursor_events(immersive).map_err(|e| e.to_string())?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let old_size = window.outer_size().map_err(|e| e.to_string())?.to_logical::<f64>(scale);
    let old_position = window.outer_position().map_err(|e| e.to_string())?.to_logical::<f64>(scale);
    if (old_size.width - width).abs() > 0.5 || (old_size.height - height).abs() > 0.5 {
        // Separate position and size calls create two compositor frames on Windows.
        // Apply both together so the island never visibly jumps while resizing.
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_DEFERERASE, SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_NOZORDER};
            let new_position = LogicalPosition::new(old_position.x + (old_size.width - width) / 2.0, old_position.y);
            let physical_position = new_position.to_physical::<i32>(scale);
            let physical_size = LogicalSize::new(width, height).to_physical::<u32>(scale);
            unsafe {
                SetWindowPos(
                    window.hwnd().map_err(|e| e.to_string())?,
                    None,
                    physical_position.x,
                    physical_position.y,
                    physical_size.width as i32,
                    physical_size.height as i32,
                    // Keep the existing WebView frame while its bounds change;
                    // otherwise transparent windows may briefly erase the bar text.
                    SWP_DEFERERASE | SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOZORDER,
                ).map_err(|e| e.to_string())?;
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let new_position = LogicalPosition::new(old_position.x + (old_size.width - width) / 2.0, old_position.y);
            window.set_position(Position::Logical(new_position)).map_err(|e| e.to_string())?;
            window.set_size(Size::Logical(LogicalSize::new(width, height))).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
#[cfg(target_os = "windows")]
#[tauri::command] fn get_immersive_state(_window: WebviewWindow) -> Result<ImmersiveState, String> {
    use windows::Win32::{Foundation::{LPARAM, RECT}, Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITOR_DEFAULTTONEAREST, MONITORINFO}, System::Threading::GetCurrentProcessId, UI::WindowsAndMessaging::{EnumWindows, GetClassNameW, GetForegroundWindow, GetWindowLongW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsZoomed, GWL_EXSTYLE, GWL_STYLE, WS_CAPTION, WS_EX_TOPMOST}};
    unsafe {
        let island_position = _window.outer_position().map_err(|e| e.to_string())?;
        let island_size = _window.outer_size().map_err(|e| e.to_string())?;
        let mut scan = ImmersiveScan { island: RECT { left: island_position.x, top: island_position.y, right: island_position.x + island_size.width as i32, bottom: island_position.y + island_size.height as i32 }, own_pid: GetCurrentProcessId(), active: false };
        let _ = EnumWindows(Some(scan_immersive_window), LPARAM(&mut scan as *mut ImmersiveScan as isize));
        if scan.active { return Ok(ImmersiveState { active: true }); }
        let foreground = GetForegroundWindow();
        if foreground.0.is_null() { return Ok(ImmersiveState { active: false }); }
        let mut foreground_pid = 0;
        GetWindowThreadProcessId(foreground, Some(&mut foreground_pid));
        if foreground_pid == GetCurrentProcessId() { return Ok(ImmersiveState { active: false }); }
        let mut title_buffer = [0u16; 256];
        let mut class_buffer = [0u16; 256];
        let title_len = GetWindowTextW(foreground, &mut title_buffer).max(0) as usize;
        let class_len = GetClassNameW(foreground, &mut class_buffer).max(0) as usize;
        let window_identity = format!("{} {}", String::from_utf16_lossy(&title_buffer[..title_len]), String::from_utf16_lossy(&class_buffer[..class_len])).to_ascii_lowercase();
        // Desktop shells and desktop-fence overlays can own the foreground while a user
        // works normally. They are not immersive application surfaces.
        if ["progman", "workerw", "desktop", "fence", "textinputhost", "windows 输入体验"].iter().any(|term| window_identity.contains(term)) {
            return Ok(ImmersiveState { active: false });
        }
        let mut foreground_rect = RECT::default();
        if GetWindowRect(foreground, &mut foreground_rect).is_err() { return Ok(ImmersiveState { active: false }); }
        let monitor = MonitorFromWindow(foreground, MONITOR_DEFAULTTONEAREST);
        let mut monitor_info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if !GetMonitorInfoW(monitor, &mut monitor_info).as_bool() { return Ok(ImmersiveState { active: false }); }
        let screen = monitor_info.rcMonitor;
        // A maximized framed window is still normal work. Full-screen means either an
        // unmaximized borderless surface or a maximized surface with its caption removed.
        let tolerance = 2;
        let fills_monitor = foreground_rect.left <= screen.left + tolerance && foreground_rect.top <= screen.top + tolerance && foreground_rect.right >= screen.right - tolerance && foreground_rect.bottom >= screen.bottom - tolerance;
        let style = GetWindowLongW(foreground, GWL_STYLE) as u32;
        let is_frameless = (style & WS_CAPTION.0) == 0;
        let is_full_screen = fills_monitor && (!IsZoomed(foreground).as_bool() || is_frameless);
        let covers_island = foreground_rect.left <= island_position.x && foreground_rect.top <= island_position.y && foreground_rect.right >= island_position.x + island_size.width as i32 && foreground_rect.bottom >= island_position.y + island_size.height as i32;
        let extended_style = GetWindowLongW(foreground, GWL_EXSTYLE) as u32;
        let is_external_topmost_overlay = (extended_style & WS_EX_TOPMOST.0) != 0;
        Ok(ImmersiveState { active: is_full_screen || (is_external_topmost_overlay && covers_island) })
    }
}
#[cfg(not(target_os = "windows"))]
#[tauri::command] fn get_immersive_state() -> Result<ImmersiveState, String> { Ok(ImmersiveState { active: false }) }
#[tauri::command] fn save_window_position(window: WebviewWindow) -> Result<(), String> {
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let position = window.outer_position().map_err(|e| e.to_string())?.to_logical::<f64>(scale);
    let path = position_file().ok_or("无法定位应用设置目录")?;
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
    std::fs::write(path, serde_json::to_string(&SavedWindowPosition { x: position.x, y: position.y, user_moved: true }).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
#[tauri::command]
fn show_detail_panel(window: WebviewWindow) -> Result<(), String> {
    let panel = window.app_handle().get_webview_window("panel").ok_or("未找到详情窗口")?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let main_position = window.outer_position().map_err(|e| e.to_string())?.to_logical::<f64>(scale);
    let main_size = window.outer_size().map_err(|e| e.to_string())?.to_logical::<f64>(scale);
    let width = 520.0;
    // Start the native hit area directly below the pill. The visible panel keeps
    // its 9px breathing gap inside this window, but the pointer never falls
    // through an unhandled gap while travelling from pill to details.
    let position = LogicalPosition::new(main_position.x + (main_size.width - width) / 2.0, main_position.y + 46.0);
    panel.set_always_on_top(true).map_err(|e| e.to_string())?;
    panel.set_ignore_cursor_events(false).map_err(|e| e.to_string())?;
    panel.set_position(Position::Logical(position)).map_err(|e| e.to_string())?;
    panel.set_size(Size::Logical(LogicalSize::new(width, 351.0))).map_err(|e| e.to_string())?;
    panel.show().map_err(|e| e.to_string())
}
#[tauri::command]
fn hide_detail_panel(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(panel) = app.get_webview_window("panel") { panel.hide().map_err(|e| e.to_string())?; }
    Ok(())
}
#[cfg(target_os = "windows")]
#[tauri::command]
fn is_cursor_over_island(app: tauri::AppHandle) -> Result<bool, String> {
    use windows::Win32::{Foundation::{POINT, RECT}, UI::WindowsAndMessaging::{GetCursorPos, GetWindowRect}};
    unsafe {
        let mut cursor = POINT::default();
        GetCursorPos(&mut cursor).map_err(|e| e.to_string())?;
        for label in ["main", "panel"] {
            let Some(window) = app.get_webview_window(label) else { continue; };
            if !window.is_visible().unwrap_or(false) { continue; }
            let mut rect = RECT::default();
            GetWindowRect(window.hwnd().map_err(|e| e.to_string())?, &mut rect).map_err(|e| e.to_string())?;
            if cursor.x >= rect.left && cursor.x < rect.right && cursor.y >= rect.top && cursor.y < rect.bottom { return Ok(true); }
        }
        Ok(false)
    }
}
#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn is_cursor_over_island() -> Result<bool, String> { Ok(false) }
#[tauri::command] fn start_window_drag(window: WebviewWindow) -> Result<(), String> { window.start_dragging().map_err(|e| e.to_string()) }
#[tauri::command] fn exit_app(app: tauri::AppHandle) { app.exit(0); }
fn show_main_window(app: &tauri::AppHandle) { if let Some(window) = app.get_webview_window("main") { let _ = window.show(); let _ = window.set_focus(); } }
pub fn run() { tauri::Builder::default().plugin(tauri_plugin_opener::init()).setup(|app| {
    if let Some(window) = app.get_webview_window("main") { let _ = window.set_always_on_top(true); let _ = restore_window_position(&window, 236.0, 46.0); }
    let show = MenuItem::with_id(app, "show", "显示 Codex Island", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "隐藏 Codex Island", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &hide, &quit])?;
    let mut tray = TrayIconBuilder::with_id("codex-island-tray").tooltip("Codex Island").menu(&menu);
    if let Some(icon) = app.default_window_icon() { tray = tray.icon(icon.clone()); }
    tray.on_menu_event(|app, event| match event.id.as_ref() { "show" => show_main_window(app), "hide" => { if let Some(window) = app.get_webview_window("main") { let _ = window.hide(); } }, "quit" => app.exit(0), _ => {} }).build(app)?;
    Ok(())
}).invoke_handler(tauri::generate_handler![fetch_usage, set_expanded, get_immersive_state, save_window_position, show_detail_panel, hide_detail_panel, is_cursor_over_island, start_window_drag, exit_app]).run(tauri::generate_context!()).expect("error while running Codex Island"); }
