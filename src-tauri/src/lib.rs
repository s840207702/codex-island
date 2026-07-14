use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu}, tray::TrayIconBuilder, Emitter, LogicalPosition, LogicalSize, Manager, Position, Size, WebviewWindow};
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
use tauri_plugin_opener::OpenerExt;

#[derive(Clone, Serialize)] struct Window { used_percent: f64, remaining_percent: f64, reset_after_seconds: i64, reset_at: Option<Value> }
#[derive(Clone, Serialize)] struct Usage { weekly: Window, plan_type: String, plan_multiplier: Option<String>, reset_credits: Option<i64>, reset_credit_expires_at: Option<Value>, credit_balance: Option<f64>, has_credits: bool, fetched_at: String }
// Legacy dual-window response, retained while OpenAI's temporary 5-hour quota removal is in effect:
// struct Usage { primary: Window, secondary: Window, ... }
#[derive(Deserialize)] struct Auth { tokens: Tokens }
#[derive(Deserialize)] struct Tokens { access_token: String, account_id: Option<String> }
#[derive(Serialize, Deserialize)] struct SavedWindowPosition { x: f64, y: f64, #[serde(default)] user_moved: bool }
#[derive(Serialize)] struct ImmersiveState { active: bool }
#[derive(Default)] struct UsageCache(std::sync::Mutex<Option<Usage>>);

fn position_file() -> Option<std::path::PathBuf> { dirs::config_dir().map(|dir| dir.join("codex-island").join("window-position.json")) }
fn language_file() -> Option<std::path::PathBuf> { dirs::config_dir().map(|dir| dir.join("codex-island").join("language.txt")) }
fn read_language() -> String {
    language_file()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|value| value.trim_matches(|character: char| character.is_whitespace() || character == '\u{feff}').to_owned())
        .filter(|value| ["zh-CN", "zh-TW", "en", "ja", "ko", "es", "fr", "de", "pt-BR", "ru"].contains(&value.as_str()))
        .unwrap_or_else(|| "zh-CN".to_owned())
}
fn write_language(language: &str) {
    if let Some(path) = language_file() { if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); } let _ = std::fs::write(path, language); }
}
#[tauri::command] fn get_app_language() -> String { read_language() }
fn sync_language_to_windows(app: &tauri::AppHandle, language: &str) {
    let payload = serde_json::to_string(language).unwrap_or_else(|_| "\"zh-CN\"".to_owned());
    let script = format!("window.dispatchEvent(new CustomEvent('codex-island-language-dom-change', {{ detail: {payload} }}));");
    for label in ["main", "panel"] {
        let _ = app.emit_to(label, "codex-island-language-change", language.to_owned());
        if let Some(window) = app.get_webview_window(label) { let _ = window.eval(&script); }
    }
}
struct TrayLabels { show: &'static str, refresh: &'static str, autostart: &'static str, language: &'static str, github: &'static str, toolbox: &'static str, quit: &'static str }
fn tray_labels(language: &str) -> TrayLabels { match language {
    "en" => TrayLabels { show: "Show Codex Island", refresh: "Refresh now", autostart: "Launch at startup", language: "Language", github: "GitHub repository", toolbox: "Feige Toolbox", quit: "Quit Codex Island" },
    "ja" => TrayLabels { show: "Codex Island を表示", refresh: "今すぐ更新", autostart: "起動時に実行", language: "言語", github: "GitHub リポジトリ", toolbox: "非哥ツールボックス", quit: "Codex Island を終了" },
    "ko" => TrayLabels { show: "Codex Island 표시", refresh: "지금 새로고침", autostart: "시작 시 실행", language: "언어", github: "GitHub 저장소", toolbox: "非哥 도구 상자", quit: "Codex Island 종료" },
    "zh-TW" => TrayLabels { show: "顯示 Codex Island", refresh: "立即重新整理", autostart: "開機時啟動", language: "語言", github: "GitHub 儲存庫", toolbox: "非哥工具箱", quit: "結束 Codex Island" },
    "es" => TrayLabels { show: "Mostrar Codex Island", refresh: "Actualizar ahora", autostart: "Iniciar con el sistema", language: "Idioma", github: "Repositorio de GitHub", toolbox: "Feige Toolbox", quit: "Salir de Codex Island" },
    "fr" => TrayLabels { show: "Afficher Codex Island", refresh: "Actualiser maintenant", autostart: "Lancer au démarrage", language: "Langue", github: "Dépôt GitHub", toolbox: "Feige Toolbox", quit: "Quitter Codex Island" },
    "de" => TrayLabels { show: "Codex Island anzeigen", refresh: "Jetzt aktualisieren", autostart: "Beim Start ausführen", language: "Sprache", github: "GitHub-Repository", toolbox: "Feige Toolbox", quit: "Codex Island beenden" },
    "pt-BR" => TrayLabels { show: "Mostrar Codex Island", refresh: "Atualizar agora", autostart: "Iniciar com o sistema", language: "Idioma", github: "Repositório GitHub", toolbox: "Feige Toolbox", quit: "Sair do Codex Island" },
    "ru" => TrayLabels { show: "Показать Codex Island", refresh: "Обновить сейчас", autostart: "Запускать при старте", language: "Язык", github: "Репозиторий GitHub", toolbox: "Feige Toolbox", quit: "Выйти из Codex Island" },
    _ => TrayLabels { show: "显示 Codex Island", refresh: "立即刷新", autostart: "开机时启动", language: "语言", github: "GitHub 仓库", toolbox: "非哥工具箱", quit: "退出 Codex Island" },
} }

fn parse_window(v: &Value) -> Result<Window, String> {
    let used = v.get("used_percent").and_then(Value::as_f64).unwrap_or(0.0).clamp(0.0, 100.0);
    Ok(Window { used_percent: used, remaining_percent: 100.0 - used, reset_after_seconds: v.get("reset_after_seconds").and_then(Value::as_i64).unwrap_or(0), reset_at: v.get("reset_at").cloned() })
}
fn weekly_window(limit: &Value) -> Result<&Value, String> {
    [limit.get("primary_window"), limit.get("secondary_window")]
        .into_iter()
        .flatten()
        .filter(|window| !window.is_null())
        .max_by_key(|window| window.get("limit_window_seconds").and_then(Value::as_i64).unwrap_or(0))
        .ok_or_else(|| "缺少周额度".to_owned())
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
    // Keep the resident capsule below macOS's menu-bar/notch safe area. The
    // status-level window may overlay the menu bar, but the physical notch
    // still occludes pixels in the center, so visible content must start at
    // the monitor work-area origin on macOS.
    let top = if cfg!(target_os = "macos") { monitor.work_area().position.to_logical::<f64>(scale).y } else { position.y };
    window.set_position(Position::Logical(LogicalPosition::new(position.x + (size.width - width) / 2.0, top))).map_err(|e| e.to_string())?;
    window.set_size(Size::Logical(LogicalSize::new(width, height))).map_err(|e| e.to_string())
}
#[cfg(target_os = "macos")]
fn set_island_window_level(window: &WebviewWindow) -> Result<(), String> {
    window.with_webview(|webview| unsafe {
        let native_window: &objc2_app_kit::NSWindow = &*webview.ns_window().cast();
        native_window.setLevel(objc2_app_kit::NSStatusWindowLevel);
    }).map_err(|e| e.to_string())
}
#[cfg(not(target_os = "macos"))]
fn set_island_window_level(window: &WebviewWindow) -> Result<(), String> {
    window.set_always_on_top(true).map_err(|e| e.to_string())
}
fn restore_window_position(window: &WebviewWindow, width: f64, height: f64) -> Result<(), String> {
    // Position is deliberately session-only: every new launch starts at the
    // top-center of the active display, while in-session dragging remains free.
    center_window(window, width, height)
}
#[tauri::command]
async fn fetch_usage(window: WebviewWindow, cache: tauri::State<'_, UsageCache>) -> Result<Usage, String> {
    // The main island owns network refreshes. The hidden detail WebView reads the
    // same successful snapshot so WebView2 timer throttling cannot split the UI.
    if window.label() == "panel" {
        if let Some(usage) = cache.0.lock().ok().and_then(|slot| slot.clone()) { return Ok(usage); }
    }
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
    // Legacy dual-window mapping (restore if the 5-hour quota returns):
    // primary: parse_window(limit.get("primary_window").ok_or("缺少短期额度")?)?,
    // secondary: parse_window(limit.get("secondary_window").ok_or("缺少周额度")?)?,
    let usage = Usage { weekly: parse_window(weekly_window(limit)?)?, plan_type: body.get("plan_type").and_then(Value::as_str).unwrap_or("unknown").to_owned(), plan_multiplier: body.get("promo").and_then(|p| p.get("multiplier").or_else(|| p.get("rate_limit_multiplier"))).and_then(Value::as_str).map(str::to_owned), reset_credits: ["available_count", "availableCount", "remaining", "count"].iter().find_map(|key| reset.get(*key).and_then(Value::as_i64)), reset_credit_expires_at, credit_balance: credits.get("balance").and_then(Value::as_f64), has_credits: credits.get("has_credits").and_then(Value::as_bool).unwrap_or(false), fetched_at: chrono_like_now() };
    if let Ok(mut slot) = cache.0.lock() { *slot = Some(usage.clone()); }
    Ok(usage)
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
        (
            content_width.filter(|value| *value >= 100.0).unwrap_or(fallback_width),
            content_height.filter(|value| *value >= 30.0).unwrap_or(fallback_height),
        )
    };
    // The React layout uses CSS pixels. Logical sizing keeps that layout stable
    // at 100%, 125%, 150%, and 200% Windows DPI scaling.
    set_island_window_level(&window)?;
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
    use windows::Win32::{Foundation::{POINT, RECT}, Graphics::Gdi::{ClientToScreen, GetMonitorInfoW, MonitorFromWindow, MONITOR_DEFAULTTONEAREST, MONITORINFO}, System::Threading::GetCurrentProcessId, UI::WindowsAndMessaging::{GetClassNameW, GetClientRect, GetForegroundWindow, GetWindowLongW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsZoomed, GWL_STYLE, WS_CAPTION}};
    unsafe {
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
        // Chromium, Electron, and some players keep WS_CAPTION while in full
        // screen. Their client area still fills the monitor, unlike a normal
        // maximized window whose client area excludes chrome or the taskbar.
        let mut client_rect = RECT::default();
        let client_fills_monitor = if GetClientRect(foreground, &mut client_rect).is_ok() {
            let mut client_top_left = POINT { x: client_rect.left, y: client_rect.top };
            let mut client_bottom_right = POINT { x: client_rect.right, y: client_rect.bottom };
            ClientToScreen(foreground, &mut client_top_left).as_bool()
                && ClientToScreen(foreground, &mut client_bottom_right).as_bool()
                && client_top_left.x <= screen.left + tolerance
                && client_top_left.y <= screen.top + tolerance
                && client_bottom_right.x >= screen.right - tolerance
                && client_bottom_right.y >= screen.bottom - tolerance
        } else { false };
        let is_full_screen = fills_monitor && (!IsZoomed(foreground).as_bool() || is_frameless || client_fills_monitor);
        // Normal maximized and topmost windows must never hide the island.
        // Immersive mode is reserved for a genuine foreground full-screen surface.
        Ok(ImmersiveState { active: is_full_screen })
    }
}
#[cfg(target_os = "macos")]
#[tauri::command]
fn get_immersive_state(window: WebviewWindow) -> Result<ImmersiveState, String> {
    use core_foundation::{array::CFArray, base::{CFType, TCFType}, dictionary::CFDictionary, number::CFNumber};
    use core_graphics::{geometry::CGRect, window::{CGWindowListCopyWindowInfo, kCGWindowBounds, kCGWindowLayer, kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenOnly, kCGWindowOwnerPID}};
    use std::ffi::c_void;

    fn value(dict: &CFDictionary, key: *const c_void) -> Option<CFType> {
        let raw = *dict.find(key)?;
        if raw.is_null() { return None; }
        Some(unsafe { CFType::wrap_under_get_rule(raw as _) })
    }

    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let island_position = window.outer_position().map_err(|e| e.to_string())?.to_logical::<f64>(scale);
    let island_size = window.outer_size().map_err(|e| e.to_string())?.to_logical::<f64>(scale);
    // The native main window widens to 520px while details or immersive visuals
    // are active. Coverage must follow the actual resident capsule, not those
    // transparent side gutters, so keep a stable 236x46 target around its center.
    let capsule_left = island_position.x + (island_size.width - 236.0) / 2.0;
    let capsule_top = island_position.y;
    let capsule_right = capsule_left + 236.0;
    let capsule_bottom = capsule_top + 46.0;

    let options = kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements;
    let raw_windows = unsafe { CGWindowListCopyWindowInfo(options, 0) };
    if raw_windows.is_null() { return Ok(ImmersiveState { active: false }); }
    let windows: CFArray<CFDictionary> = unsafe { TCFType::wrap_under_create_rule(raw_windows) };
    let owner_key = unsafe { kCGWindowOwnerPID as *const c_void };
    let layer_key = unsafe { kCGWindowLayer as *const c_void };
    let bounds_key = unsafe { kCGWindowBounds as *const c_void };
    let own_pid = std::process::id() as f64;

    // Quartz is ordered front-to-back. Pick the first foreign layer-zero owner
    // that has a window large enough to contain the whole capsule. This skips
    // cursor/automation helper overlays while still allowing a small ordinary
    // app window to trigger once it genuinely covers the capsule. Then consider
    // all of that owner's windows because its first entry can be a title helper.
    let foreground_pid = windows.iter().find_map(|candidate| {
        let owner_pid = value(&candidate, owner_key)?.downcast::<CFNumber>()?.to_f64()?;
        if (owner_pid - own_pid).abs() < 0.5 { return None; }
        let layer = value(&candidate, layer_key)?.downcast::<CFNumber>()?.to_f64()?;
        if layer.abs() > 0.5 { return None; }
        let bounds = value(&candidate, bounds_key)?.downcast::<CFDictionary>()?;
        let rect = CGRect::from_dict_representation(&bounds)?;
        if rect.size.width < 236.0 || rect.size.height < 46.0 { return None; }
        Some(owner_pid)
    });
    let Some(foreground_pid) = foreground_pid else { return Ok(ImmersiveState { active: false }); };

    let tolerance = 1.5;
    let active = windows.iter().any(|candidate| {
        let Some(owner_pid) = value(&candidate, owner_key).and_then(|item| item.downcast::<CFNumber>()).and_then(|item| item.to_f64()) else { return false; };
        if (owner_pid - foreground_pid).abs() >= 0.5 { return false; }
        let Some(layer) = value(&candidate, layer_key).and_then(|item| item.downcast::<CFNumber>()).and_then(|item| item.to_f64()) else { return false; };
        if layer.abs() > 0.5 { return false; }
        let Some(bounds) = value(&candidate, bounds_key).and_then(|item| item.downcast::<CFDictionary>()) else { return false; };
        let Some(rect) = CGRect::from_dict_representation(&bounds) else { return false; };
        let right = rect.origin.x + rect.size.width;
        let bottom = rect.origin.y + rect.size.height;
        rect.origin.x <= capsule_left + tolerance
            && rect.origin.y <= capsule_top + tolerance
            && right >= capsule_right - tolerance
            && bottom >= capsule_bottom - tolerance
    });

    Ok(ImmersiveState { active })
}
#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
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
    position_detail_panel(&window, &panel)?;
    panel.show().map_err(|e| e.to_string())?;
    let _ = panel.eval("window.dispatchEvent(new Event('codex-island-panel-shown')); ");
    // The details webview spends most of its lifetime hidden. Re-sync its
    // locale whenever it becomes visible so a missed background event can
    // never leave the pill and panel in different languages.
    sync_language_to_windows(window.app_handle(), &read_language());
    Ok(())
}
fn position_detail_panel(window: &WebviewWindow, panel: &WebviewWindow) -> Result<(), String> {
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let main_position = window.outer_position().map_err(|e| e.to_string())?.to_logical::<f64>(scale);
    let main_size = window.outer_size().map_err(|e| e.to_string())?.to_logical::<f64>(scale);
    let width = 520.0;
    // Start the native hit area directly below the pill. The visible panel keeps
    // its 9px breathing gap inside this window, but the pointer never falls
    // through an unhandled gap while travelling from pill to details.
    let position = LogicalPosition::new(main_position.x + (main_size.width - width) / 2.0, main_position.y + 46.0);
    set_island_window_level(&panel)?;
    panel.set_ignore_cursor_events(false).map_err(|e| e.to_string())?;
    panel.set_position(Position::Logical(position)).map_err(|e| e.to_string())?;
    panel.set_size(Size::Logical(LogicalSize::new(width, 351.0))).map_err(|e| e.to_string())
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
#[cfg(target_os = "macos")]
#[tauri::command]
fn is_cursor_over_island(app: tauri::AppHandle) -> Result<bool, String> {
    use core_graphics::{event::CGEvent, event_source::{CGEventSource, CGEventSourceStateID}};
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).map_err(|_| "无法读取 macOS 鼠标状态")?;
    let cursor = CGEvent::new(source).map_err(|_| "无法读取 macOS 鼠标状态")?.location();
    for label in ["main", "panel"] {
        let Some(window) = app.get_webview_window(label) else { continue; };
        if !window.is_visible().unwrap_or(false) { continue; }
        let scale = window.scale_factor().map_err(|e| e.to_string())?;
        let position = window.outer_position().map_err(|e| e.to_string())?.to_logical::<f64>(scale);
        let size = window.outer_size().map_err(|e| e.to_string())?.to_logical::<f64>(scale);
        // When expanded, the main native window is widened to center the
        // separate details window. Only the visible 236x46 capsule should
        // keep the island open; its transparent side gutters must not count
        // as a hover target on macOS.
        let (left, top, width, height) = if label == "main" && size.width > 300.0 {
            (position.x + (size.width - 236.0) / 2.0, position.y, 236.0, 46.0)
        } else {
            (position.x, position.y, size.width, size.height)
        };
        if cursor.x >= left && cursor.x < left + width && cursor.y >= top && cursor.y < top + height { return Ok(true); }
    }
    Ok(false)
}
#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
#[tauri::command]
fn is_cursor_over_island() -> Result<bool, String> { Ok(false) }
#[tauri::command] fn start_window_drag(window: WebviewWindow) -> Result<(), String> { window.start_dragging().map_err(|e| e.to_string()) }
#[tauri::command] fn exit_app(app: tauri::AppHandle) { app.exit(0); }
fn show_main_window(app: &tauri::AppHandle) { if let Some(window) = app.get_webview_window("main") { let _ = center_window(&window, 236.0, 46.0); let _ = window.show(); let _ = window.set_focus(); } }
pub fn run() { tauri::Builder::default()
    .manage(UsageCache::default())
    .plugin(tauri_plugin_opener::init())
    .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
    .setup(|app| {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    if let Some(window) = app.get_webview_window("main") {
        let _ = set_island_window_level(&window);
        let _ = restore_window_position(&window, 236.0, 46.0);
        let app_handle = app.handle().clone();
        window.on_window_event(move |event| {
            if matches!(event, tauri::WindowEvent::Moved(_)) {
                if let (Some(main), Some(panel)) = (app_handle.get_webview_window("main"), app_handle.get_webview_window("panel")) {
                    if panel.is_visible().unwrap_or(false) { let _ = position_detail_panel(&main, &panel); }
                }
            }
        });
    }
    let language = read_language();
    let labels = tray_labels(&language);
    let show = CheckMenuItem::with_id(app, "show", labels.show, true, true, None::<&str>)?;
    let refresh = MenuItem::with_id(app, "refresh", labels.refresh, true, None::<&str>)?;
    let autostart = CheckMenuItem::with_id(app, "autostart", labels.autostart, true, app.autolaunch().is_enabled().unwrap_or(false), None::<&str>)?;
    let lang_zh = CheckMenuItem::with_id(app, "lang-zh-CN", "简体中文", true, language == "zh-CN", None::<&str>)?;
    let lang_en = CheckMenuItem::with_id(app, "lang-en", "English", true, language == "en", None::<&str>)?;
    let lang_ja = CheckMenuItem::with_id(app, "lang-ja", "日本語", true, language == "ja", None::<&str>)?;
    let lang_ko = CheckMenuItem::with_id(app, "lang-ko", "한국어", true, language == "ko", None::<&str>)?;
    let lang_zh_tw = CheckMenuItem::with_id(app, "lang-zh-TW", "繁體中文", true, language == "zh-TW", None::<&str>)?;
    let lang_es = CheckMenuItem::with_id(app, "lang-es", "Español", true, language == "es", None::<&str>)?;
    let lang_fr = CheckMenuItem::with_id(app, "lang-fr", "Français", true, language == "fr", None::<&str>)?;
    let lang_de = CheckMenuItem::with_id(app, "lang-de", "Deutsch", true, language == "de", None::<&str>)?;
    let lang_pt = CheckMenuItem::with_id(app, "lang-pt-BR", "Português (Brasil)", true, language == "pt-BR", None::<&str>)?;
    let lang_ru = CheckMenuItem::with_id(app, "lang-ru", "Русский", true, language == "ru", None::<&str>)?;
    let language_menu = Submenu::with_items(app, labels.language, true, &[&lang_zh, &lang_zh_tw, &lang_en, &lang_ja, &lang_ko, &lang_es, &lang_fr, &lang_de, &lang_pt, &lang_ru])?;
    let github = MenuItem::with_id(app, "github", labels.github, true, None::<&str>)?;
    let toolbox = MenuItem::with_id(app, "toolbox", labels.toolbox, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", labels.quit, true, None::<&str>)?;
    let separator_one = PredefinedMenuItem::separator(app)?;
    let separator_two = PredefinedMenuItem::separator(app)?;
    let separator_three = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&show, &refresh, &separator_one, &autostart, &language_menu, &separator_two, &github, &toolbox, &separator_three, &quit])?;
    let mut tray = TrayIconBuilder::with_id("codex-island-tray").tooltip("Codex Island").menu(&menu);
    tray = tray.icon(tauri::image::Image::from_bytes(include_bytes!("../icons/tray-avatar.png"))?);
    let show_item = show.clone();
    let autostart_item = autostart.clone();
    let language_menu_item = language_menu.clone();
    let refresh_item = refresh.clone();
    let github_item = github.clone();
    let toolbox_item = toolbox.clone();
    let quit_item = quit.clone();
    let zh_item = lang_zh.clone(); let zh_tw_item = lang_zh_tw.clone(); let en_item = lang_en.clone(); let ja_item = lang_ja.clone(); let ko_item = lang_ko.clone();
    let es_item = lang_es.clone(); let fr_item = lang_fr.clone(); let de_item = lang_de.clone(); let pt_item = lang_pt.clone(); let ru_item = lang_ru.clone();
    tray.on_menu_event(move |app, event| match event.id.as_ref() {
        "show" => {
            if show_item.is_checked().unwrap_or(true) { show_main_window(app); }
            else { if let Some(window) = app.get_webview_window("main") { let _ = window.hide(); } if let Some(panel) = app.get_webview_window("panel") { let _ = panel.hide(); } }
        },
        "refresh" => { let _ = app.emit("codex-island-refresh", ()); },
        "autostart" => {
            let enabled = autostart_item.is_checked().unwrap_or(false);
            let result = if enabled { app.autolaunch().enable() } else { app.autolaunch().disable() };
            if result.is_err() { let _ = autostart_item.set_checked(!enabled); }
        },
        id if id.starts_with("lang-") => {
            let language = event.id.as_ref().trim_start_matches("lang-");
            let _ = zh_item.set_checked(language == "zh-CN"); let _ = zh_tw_item.set_checked(language == "zh-TW"); let _ = en_item.set_checked(language == "en"); let _ = ja_item.set_checked(language == "ja"); let _ = ko_item.set_checked(language == "ko");
            let _ = es_item.set_checked(language == "es"); let _ = fr_item.set_checked(language == "fr"); let _ = de_item.set_checked(language == "de"); let _ = pt_item.set_checked(language == "pt-BR"); let _ = ru_item.set_checked(language == "ru");
            write_language(language);
            let labels = tray_labels(language);
            let _ = show_item.set_text(labels.show); let _ = refresh_item.set_text(labels.refresh); let _ = autostart_item.set_text(labels.autostart); let _ = language_menu_item.set_text(labels.language); let _ = github_item.set_text(labels.github); let _ = toolbox_item.set_text(labels.toolbox); let _ = quit_item.set_text(labels.quit);
            sync_language_to_windows(app, language);
        },
        "github" => { let _ = app.opener().open_url("https://github.com/s840207702/codex-island", None::<&str>); },
        "toolbox" => { let _ = app.opener().open_url("https://www.feige177.com", None::<&str>); },
        "quit" => app.exit(0),
        _ => {}
    }).build(app)?;
    Ok(())
}).invoke_handler(tauri::generate_handler![fetch_usage, get_app_language, set_expanded, get_immersive_state, save_window_position, show_detail_panel, hide_detail_panel, is_cursor_over_island, start_window_drag, exit_app]).run(tauri::generate_context!()).expect("error while running Codex Island"); }
