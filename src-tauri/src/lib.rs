use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{LogicalSize, Size, WebviewWindow};

#[derive(Serialize)] struct Window { used_percent: f64, remaining_percent: f64, reset_after_seconds: i64, reset_at: Option<Value> }
#[derive(Serialize)] struct Usage { primary: Window, secondary: Window, credit_balance: Option<f64>, has_credits: bool, fetched_at: String }
#[derive(Deserialize)] struct Auth { tokens: Tokens }
#[derive(Deserialize)] struct Tokens { access_token: String, account_id: Option<String> }

fn parse_window(v: &Value) -> Result<Window, String> {
    let used = v.get("used_percent").and_then(Value::as_f64).unwrap_or(0.0).clamp(0.0, 100.0);
    Ok(Window { used_percent: used, remaining_percent: 100.0 - used, reset_after_seconds: v.get("reset_after_seconds").and_then(Value::as_i64).unwrap_or(0), reset_at: v.get("reset_at").cloned() })
}
#[tauri::command]
async fn fetch_usage() -> Result<Usage, String> {
    let path = dirs::home_dir().ok_or("无法定位用户目录")?.join(".codex").join("auth.json");
    let auth: Auth = serde_json::from_str(&std::fs::read_to_string(path).map_err(|_| "未找到 Codex 登录态，请先登录 Codex")?).map_err(|_| "Codex 登录态格式无效")?;
    let client = reqwest::Client::new();
    let mut request = client.get("https://chatgpt.com/backend-api/wham/usage").bearer_auth(auth.tokens.access_token).header("User-Agent", "CodexQuotaIsland/0.1 (local-only)");
    if let Some(id) = auth.tokens.account_id { request = request.header("ChatGPT-Account-ID", id); }
    let body: Value = request.send().await.map_err(|_| "无法连接 OpenAI 额度接口")?.error_for_status().map_err(|e| format!("OpenAI 额度接口错误：{}", e.status().map(|x| x.as_u16()).unwrap_or(0)))?.json().await.map_err(|_| "OpenAI 返回的额度数据无法解析")?;
    let limit = body.get("rate_limit").ok_or("OpenAI 未返回额度窗口")?;
    let credits = body.get("credits").unwrap_or(&Value::Null);
    Ok(Usage { primary: parse_window(limit.get("primary_window").ok_or("缺少短期额度")?)?, secondary: parse_window(limit.get("secondary_window").ok_or("缺少周额度")?)?, credit_balance: credits.get("balance").and_then(Value::as_f64), has_credits: credits.get("has_credits").and_then(Value::as_bool).unwrap_or(false), fetched_at: chrono_like_now() })
}
fn chrono_like_now() -> String { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs().to_string() }
#[tauri::command] fn set_pinned(window: WebviewWindow, pinned: bool) -> Result<(), String> { window.set_always_on_top(pinned).map_err(|e| e.to_string()) }
#[tauri::command] fn set_expanded(window: WebviewWindow, expanded: bool) -> Result<(), String> {
    let (width, height) = if expanded { (540, 390) } else { (300, 64) };
    // The React layout uses CSS pixels. Logical sizing keeps that layout stable
    // at 100%, 125%, 150%, and 200% Windows DPI scaling.
    window.set_size(Size::Logical(LogicalSize::new(width as f64, height as f64))).map_err(|e| e.to_string())
}
#[tauri::command] fn hide_window(window: WebviewWindow) -> Result<(), String> { window.hide().map_err(|e| e.to_string()) }
pub fn run() { tauri::Builder::default().plugin(tauri_plugin_opener::init()).invoke_handler(tauri::generate_handler![fetch_usage, set_pinned, set_expanded, hide_window]).run(tauri::generate_context!()).expect("error while running Codex Quota Island"); }
