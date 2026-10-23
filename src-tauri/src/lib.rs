//! 桌面生命周期与 IPC。所有变更按 running → store 的固定锁顺序执行；
//! 网络等待不持有互斥锁，应用内一次只能有一个付费测试请求。作者：ahui。

pub mod api;
mod autostart;
mod discovery;
pub mod model;
pub mod storage;
mod updates;

use chrono::Utc;
use keyring::Entry;
use model::*;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Mutex, MutexGuard,
};
use std::time::Instant;
use storage::Store;
use tauri::{
    Emitter, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const CREDENTIAL_SERVICE: &str = "com.ahui.model-probe";
const SHORTCUT: &str = "ctrl+alt+p";

/// 运行任务拥有自己的取消令牌；仅在历史持久化完成后释放占用。
struct Running {
    info: ActiveRun,
    token: CancellationToken,
}

struct AppState {
    store: Mutex<Store>,
    running: Mutex<Option<Running>>,
    move_generation: AtomicU64,
    settings_gate: Mutex<()>,
    layout: Mutex<WidgetLayout>,
    dragging: AtomicBool,
    next_auto_test: Mutex<Instant>,
}

/// 仅保存本次进程的边缘呈现状态；数据库仍保存完整面板位置，重启可直接操作。
#[derive(Default)]
struct WidgetLayout {
    side: Option<String>,
    tucked: bool,
    expanded: bool,
    anchor: Option<(i32, i32)>,
}

/// 事件不携带配置或凭据；前端据此切换边缘图标并显示最新测试颜色。
#[derive(Clone, serde::Serialize)]
struct DockNotice {
    side: Option<String>,
    tucked: bool,
}

/// 锁中毒时拒绝继续写配置，避免把部分更新视作成功。
fn lock<T>(mutex: &Mutex<T>) -> Result<MutexGuard<'_, T>, String> {
    mutex
        .lock()
        .map_err(|_| "本地状态异常，请退出应用后重新打开。".into())
}

/// 所有写命令共用运行态门禁；调用者须保持 guard 到写入完成。
fn require_idle(state: &AppState) -> Result<MutexGuard<'_, Option<Running>>, String> {
    let guard = lock(&state.running)?;
    if guard.is_some() {
        return Err("测试正在进行，请先完成或取消。".into());
    }
    Ok(guard)
}

/// 事件只携带刷新信号，不广播敏感配置或密钥。
fn changed(app: &tauri::AppHandle) {
    let _ = app.emit("snapshot-changed", ());
}

fn credential(id: &str) -> Result<Entry, String> {
    Entry::new(CREDENTIAL_SERVICE, id).map_err(|_| "无法访问 Windows 凭据管理器。".into())
}

/// 区分“没有凭据”和“无法读取”，禁止用空密钥静默降级已配置的鉴权。
fn read_key(entry: &Entry) -> Result<Option<String>, String> {
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err("无法读取已保存的 API Key，请检查 Windows 凭据管理器。".into()),
    }
}

fn set_key(entry: &Entry, key: Option<&str>) -> Result<(), String> {
    match key {
        Some(value) => entry
            .set_password(value)
            .map_err(|_| "API Key 保存失败，配置未更新。".into()),
        None => match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err("移除 API Key 失败，配置未更新。".into()),
        },
    }
}

#[tauri::command]
fn get_snapshot(state: State<'_, AppState>) -> Result<AppSnapshot, String> {
    let running = lock(&state.running)?;
    let store = lock(&state.store)?;
    Ok(AppSnapshot {
        config: store.config.clone(),
        history: store.history()?,
        running: running.as_ref().map(|run| run.info.clone()),
        auto_test_remaining_ms: if store.config.preferences.auto_test_enabled
            && store
                .config
                .profiles
                .iter()
                .any(|profile| Some(&profile.id) == store.config.selected_profile_id.as_ref())
        {
            Some(
                lock(&state.next_auto_test)?
                    .saturating_duration_since(Instant::now())
                    .as_millis() as u64,
            )
        } else {
            None
        },
    })
}

/// 临时草稿诊断只读凭据，不保存表单；改域名时不得隐式发送旧配置的密钥。
fn diagnostic_key(state: &AppState, input: &ProfileInput) -> Result<String, String> {
    api::endpoint(&input.base_url)?;
    if input.api_key.len() > 2500 || input.api_key.contains(['\r', '\n']) {
        return Err("API Key 太长或包含换行。".into());
    }
    if input.remove_key && !input.api_key.is_empty() {
        return Err("移除密钥与输入新密钥不能同时使用。".into());
    }
    if !input.api_key.is_empty() {
        return Ok(input.api_key.clone());
    }
    if input.remove_key || input.id.is_empty() {
        return Ok(String::new());
    }
    let store = lock(&state.store)?;
    let original = store
        .config
        .profiles
        .iter()
        .find(|p| p.id == input.id)
        .ok_or("原配置不存在，请重新选择。")?;
    if original.has_key && api::endpoint(&original.base_url)? != api::endpoint(&input.base_url)? {
        return Err("接口地址已修改，请明确输入用于新地址的密钥后重试。".into());
    }
    if original.has_key {
        read_key(&credential(&input.id)?)?.ok_or("原配置密钥不存在，请重新输入。".into())
    } else {
        Ok(String::new())
    }
}

/// 拉取模型仅发 GET，不消耗测智周期，不写入历史；诊断门禁跨窗口去重。
#[tauri::command]
async fn fetch_models(
    state: State<'_, AppState>,
    input: ProfileInput,
) -> Result<Vec<discovery::FetchedModel>, String> {
    let _guard = discovery::DiagnosticGuard::acquire()?;
    let key = diagnostic_key(&state, &input)?;
    discovery::fetch_models(&input.base_url, &key).await
}

/// 独立固定提示词连接测试，使用已保存的测智请求超时；不更改配置、周期和结果。
#[tauri::command]
async fn test_connection(
    state: State<'_, AppState>,
    input: ProfileInput,
) -> Result<discovery::ConnectionResult, String> {
    let _guard = discovery::DiagnosticGuard::acquire()?;
    let key = diagnostic_key(&state, &input)?;
    // 请求开始时取同一份持久化超时；不在网络等待期间持有存储锁。
    let timeout = lock(&state.store)?.config.preferences.timeout_seconds;
    discovery::test_connection(&input.base_url, &input.model, &key, timeout).await
}

/// 保存接口：凭据操作先行；若数据库提交失败，尝试恢复原凭据并明确报告回滚失败。
#[tauri::command]
fn save_profile(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: ProfileInput,
) -> Result<(), String> {
    let _idle = require_idle(&state)?;
    if input.name.trim().is_empty()
        || input.name.chars().count() > 80
        || input.model.trim().is_empty()
        || input.model.chars().count() > 200
        || input.base_url.len() > 2000
    {
        return Err("配置名称、模型名称或地址不符合长度要求。".into());
    }
    api::endpoint(&input.base_url)?;
    if input.api_key.len() > 2500 || input.api_key.contains(['\r', '\n']) {
        return Err("API Key 太长或包含换行，请检查粘贴内容。".into());
    }
    if input.remove_key && !input.api_key.is_empty() {
        return Err("请在填写新密钥和移除密钥之间选择一种操作。".into());
    }
    let mut store = lock(&state.store)?;
    let mut config = store.config.clone();
    let id = if input.id.is_empty() {
        Uuid::new_v4().to_string()
    } else {
        if !config.profiles.iter().any(|profile| profile.id == input.id) {
            return Err("该配置已不存在，请重新选择。".into());
        }
        input.id
    };
    let entry = credential(&id)?;
    let old_key = read_key(&entry)?;
    let next_key = if input.remove_key {
        None
    } else if input.api_key.is_empty() {
        old_key.clone()
    } else {
        Some(input.api_key)
    };
    set_key(&entry, next_key.as_deref())?;
    let profile = ModelProfile {
        id: id.clone(),
        name: input.name.trim().into(),
        base_url: input.base_url.trim().trim_end_matches('/').into(),
        model: input.model.trim().into(),
        has_key: next_key.is_some(),
    };
    if let Some(previous) = config.profiles.iter_mut().find(|item| item.id == id) {
        *previous = profile;
    } else {
        config.profiles.push(profile);
    }
    config.selected_profile_id = Some(id);
    if let Err(message) = store.save_config(config) {
        if set_key(&entry, old_key.as_deref()).is_err() {
            return Err(format!("{message} 凭据回滚也失败，请重新保存该配置。"));
        }
        return Err(message);
    }
    changed(&app);
    Ok(())
}

#[tauri::command]
fn delete_profile(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let _idle = require_idle(&state)?;
    let mut store = lock(&state.store)?;
    let mut config = store.config.clone();
    if !config.profiles.iter().any(|profile| profile.id == id) {
        return Err("该配置已不存在。".into());
    }
    let entry = credential(&id)?;
    let old_key = read_key(&entry)?;
    set_key(&entry, None)?;
    config.profiles.retain(|profile| profile.id != id);
    if config.selected_profile_id.as_deref() == Some(&id) {
        config.selected_profile_id = config.profiles.first().map(|profile| profile.id.clone());
    }
    if let Err(message) = store.save_config(config) {
        if set_key(&entry, old_key.as_deref()).is_err() {
            return Err(format!("{message} 凭据恢复失败，请重新配置密钥。"));
        }
        return Err(message);
    }
    changed(&app);
    Ok(())
}

#[tauri::command]
fn select_profile(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let _idle = require_idle(&state)?;
    let mut store = lock(&state.store)?;
    let mut config = store.config.clone();
    if !config.profiles.iter().any(|item| item.id == id) {
        return Err("所选模型配置不存在。".into());
    }
    config.selected_profile_id = Some(id);
    store.save_config(config)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
fn save_rule(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    rule: TestRule,
) -> Result<(), String> {
    let _idle = require_idle(&state)?;
    rule.validate()?;
    let mut store = lock(&state.store)?;
    let mut config = store.config.clone();
    config.rule = rule;
    store.save_config(config)?;
    changed(&app);
    Ok(())
}

/// 快捷键和置顶变更若失败，会撤回数据库写入前的运行时设置。
#[tauri::command]
fn save_preferences(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    preferences: Preferences,
) -> Result<(), String> {
    let _idle = require_idle(&state)?;
    if !(80..=100).contains(&preferences.opacity)
        || !(10..=300).contains(&preferences.timeout_seconds)
        || !(1..=1440).contains(&preferences.auto_test_interval_minutes)
    {
        return Err("不透明度应为 80—100%，超时应为 10—300 秒，定时间隔应为 1—1440 分钟。".into());
    }
    let mut store = lock(&state.store)?;
    let mut config = store.config.clone();
    let previous = config.preferences.clone();
    apply_preferences(&app, &preferences)?;
    config.preferences = preferences;
    if let Err(message) = store.save_config(config) {
        if apply_preferences(&app, &previous).is_err() {
            return Err(format!(
                "{message} 运行时设置恢复失败，请重新保存悬浮设置或重启应用。"
            ));
        }
        return Err(message);
    }
    *lock(&state.next_auto_test)? = Instant::now()
        + std::time::Duration::from_secs(store.config.preferences.auto_test_interval_minutes * 60);
    let restore_dock = !store.config.preferences.snap_to_edge;
    drop(store);
    if restore_dock {
        let expanded = lock(&state.layout)?.expanded;
        set_widget_layout(app.clone(), expanded, 0, false)?;
        let mut layout = lock(&state.layout)?;
        layout.side = None;
        drop(layout);
        let _ = app.emit(
            "widget-dock",
            DockNotice {
                side: None,
                tucked: false,
            },
        );
    }
    changed(&app);
    Ok(())
}

fn apply_preferences(app: &tauri::AppHandle, preferences: &Preferences) -> Result<(), String> {
    let shortcut = app.global_shortcut();
    let registered = shortcut.is_registered(SHORTCUT);
    if preferences.shortcut_enabled && !registered {
        shortcut
            .register(SHORTCUT)
            .map_err(|_| "快捷键 Ctrl+Alt+P 已被占用，请关闭该选项或释放快捷键。")?;
    } else if !preferences.shortcut_enabled && registered {
        shortcut
            .unregister(SHORTCUT)
            .map_err(|_| "无法注销快捷键。")?;
    }
    if let Some(window) = app.get_webview_window("widget") {
        if window.set_always_on_top(preferences.always_on_top).is_err() {
            // 置顶失败时恢复调用前的快捷键状态，避免界面显示启用、实际却已注销。
            let restored = if registered && !preferences.shortcut_enabled {
                shortcut.register(SHORTCUT)
            } else if !registered && preferences.shortcut_enabled {
                shortcut.unregister(SHORTCUT)
            } else {
                return Err("无法更新悬浮窗置顶状态。".into());
            };
            if restored.is_err() {
                return Err(
                    "置顶设置失败，快捷键恢复也失败，请重新保存悬浮设置或重启应用。".into(),
                );
            }
            return Err("无法更新悬浮窗置顶状态。".into());
        }
    }
    Ok(())
}

/// 手动入口：成功领取任务时，从本次开始时刻重置完整自动周期，而非等请求完成。
#[tauri::command]
async fn start_test(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    run_test(app, state, false).await
}

/// 两种入口共用请求生命周期；运行门禁内再次核对自动期限，避免手动重置与轮询抢跑。
/// 固定按 running → store → next_auto_test 加锁，网络等待不持有这些锁。
async fn run_test(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    automatic: bool,
) -> Result<(), String> {
    let (profile, rule, timeout, info, token) = {
        let mut running = require_idle(&state)?;
        let store = lock(&state.store)?;
        let config = &store.config;
        config.rule.validate()?;
        let profile = config
            .profiles
            .iter()
            .find(|profile| Some(&profile.id) == config.selected_profile_id.as_ref())
            .cloned()
            .ok_or("请先添加模型接口配置。")?;
        if !claim_test_deadline(
            &config.preferences,
            &mut *lock(&state.next_auto_test)?,
            Instant::now(),
            automatic,
        ) {
            return Ok(());
        }
        let info = ActiveRun {
            id: Uuid::new_v4().to_string(),
            profile_name: profile.name.clone(),
            model: profile.model.clone(),
            started_at: Utc::now().to_rfc3339(),
        };
        let token = CancellationToken::new();
        *running = Some(Running {
            info: info.clone(),
            token: token.clone(),
        });
        (
            profile,
            config.rule.clone(),
            config.preferences.timeout_seconds,
            info,
            token,
        )
    };
    changed(&app);
    let start = Instant::now();
    let key_result = credential(&profile.id)
        .and_then(|entry| read_key(&entry))
        .and_then(|key| {
            if profile.has_key && key.is_none() {
                Err("已保存的密钥不存在，请重新保存 API Key。".into())
            } else {
                Ok(key.unwrap_or_default())
            }
        });
    let completion = match key_result {
        Ok(key) => api::complete(&profile, &rule, &key, timeout, &token).await,
        Err(message) => api::Completion {
            answer: String::new(),
            error: Some(message),
            cancelled: false,
        },
    };
    let status = if completion.cancelled {
        "cancelled"
    } else if completion.error.is_some() {
        "error"
    } else if completion.answer.contains(&rule.keyword) {
        "passed"
    } else {
        "failed"
    };
    let record = TestRecord {
        id: info.id,
        profile_id: profile.id,
        profile_name: profile.name,
        model: profile.model,
        started_at: info.started_at,
        duration_ms: start.elapsed().as_millis() as u64,
        status: status.into(),
        answer: completion.answer,
        error: completion.error,
        prompt: rule.prompt,
        keyword: rule.keyword,
        rule_version: RULE_VERSION.into(),
    };
    let result = {
        let mut running = lock(&state.running)?;
        let result = lock(&state.store)?.save_record(&record);
        *running = None;
        result
    };
    changed(&app);
    result
}

#[tauri::command]
fn cancel_test(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(run) = lock(&state.running)?.as_ref() {
        run.token.cancel();
    }
    Ok(())
}

#[tauri::command]
fn clear_history(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let _idle = require_idle(&state)?;
    lock(&state.store)?.clear_history()?;
    changed(&app);
    Ok(())
}

/// 复用单一设置窗口，避免重复编辑窗口写入冲突；用户不保存时不提交草稿。
#[tauri::command]
async fn open_settings(app: tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let _gate = lock(&state.settings_gate)?;
    if let Some(window) = app.get_webview_window("settings") {
        window.show().map_err(|_| "无法显示设置窗口。")?;
        let _ = window.unminimize();
        return window.set_focus().map_err(|_| "无法聚焦设置窗口。".into());
    }
    // 1920×1080、100% 缩放下默认 1120×900，容纳常用设置及固定操作栏。
    // 高 DPI 或较小显示器保留 40px 工作区余量，沿用原最小尺寸。
    let (width, height) = app
        .get_webview_window("widget")
        .and_then(|window| {
            let (left, top, right, bottom) = work_area(&window).ok()?;
            let scale = window.scale_factor().ok()?;
            Some((
                1120.0_f64.min(((right - left) as f64 / scale - 40.0).max(700.0)),
                900.0_f64.min(((bottom - top) as f64 / scale - 40.0).max(520.0)),
            ))
        })
        .unwrap_or((1120.0, 900.0));
    WebviewWindowBuilder::new(
        &app,
        "settings",
        WebviewUrl::App("index.html?view=settings".into()),
    )
    .title("测智雷达 · 设置")
    .inner_size(width, height)
    .min_inner_size(700.0, 520.0)
    .decorations(false)
    .transparent(true)
    .center()
    .build()
    .map_err(|_| "无法创建设置窗口。")?;
    Ok(())
}

/// 获取真实 Windows 工作区，避开任务栏并支持多显示器和负坐标。
fn work_area(window: &WebviewWindow) -> Result<(i32, i32, i32, i32), String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
        };
        let hwnd = window.hwnd().map_err(|_| "无法获取窗口句柄。")?;
        // Windows API 只读取本进程窗口对应的显示器；结构体长度按实际类型初始化。
        unsafe {
            let monitor = MonitorFromWindow(hwnd.0 as _, MONITOR_DEFAULTTONEAREST);
            let mut info: MONITORINFO = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            if GetMonitorInfoW(monitor, &mut info) == 0 {
                return Err("无法读取显示器工作区。".into());
            }
            Ok((
                info.rcWork.left,
                info.rcWork.top,
                info.rcWork.right,
                info.rcWork.bottom,
            ))
        }
    }
    #[cfg(not(windows))]
    {
        let monitor = window
            .current_monitor()
            .map_err(|_| "无法读取显示器。")?
            .ok_or("没有可用显示器。")?;
        let pos = monitor.position();
        let size = monitor.size();
        Ok((
            pos.x,
            pos.y,
            pos.x + size.width as i32,
            pos.y + size.height as i32,
        ))
    }
}

/// 物理工作区内夹紧；开启吸附时阈值内直接贴边。无额外边距，左右顶部与图标坐标一致。
fn snapped_position(
    position: (i32, i32),
    size: (u32, u32),
    area: (i32, i32, i32, i32),
    snap: bool,
    threshold: i32,
) -> (i32, i32) {
    let (left, top, right, bottom) = area;
    let max_x = (right - size.0 as i32).max(left);
    let max_y = (bottom - size.1 as i32).max(top);
    let mut x = position.0.clamp(left, max_x);
    let mut y = position.1.clamp(top, max_y);
    if snap {
        if x - left < threshold {
            x = left;
        } else if max_x - x < threshold {
            x = max_x;
        }
        if y - top < threshold {
            y = top;
        } else if max_y - y < threshold {
            y = max_y;
        }
    }
    (x, y)
}

/// 边缘吸附使用逻辑 16px 阈值；所有情况先限制到工作区内，避免展开后超出屏幕。
#[tauri::command]
fn snap_widget(app: tauri::AppHandle) -> Result<(), String> {
    let window = app.get_webview_window("widget").ok_or("悬浮窗不存在。")?;
    let state = app.state::<AppState>();
    if lock(&state.layout)?.tucked {
        return Ok(());
    }
    let snap = lock(&state.store)?.config.preferences.snap_to_edge;
    let position = window.outer_position().map_err(|_| "无法读取窗口位置。")?;
    let size = window.outer_size().map_err(|_| "无法读取窗口尺寸。")?;
    let (left, top, right, bottom) = work_area(&window)?;
    let scale = window.scale_factor().unwrap_or(1.0);
    let threshold = (16.0 * scale) as i32;
    // 拖到边缘立即贴边，不先留 10px 再在缩成图标时跳动。
    let (x, y) = snapped_position(
        (position.x, position.y),
        (size.width, size.height),
        (left, top, right, bottom),
        snap,
        threshold,
    );
    if (x, y) != (position.x, position.y) {
        window
            .set_position(PhysicalPosition::new(x, y))
            .map_err(|_| "无法调整窗口位置。")?;
    }
    let mut store = lock(&state.store)?;
    if store.config.widget_position != Some((x, y)) {
        let mut config = store.config.clone();
        config.widget_position = Some((x, y));
        store.save_config(config)?;
    }
    Ok(())
}

/// 统一处理面板、菜单与边缘小图标的真实窗口尺寸，隐藏区不拦截桌面鼠标。
/// 完整面板两态均为 360 逻辑像素宽，与 CSS 和初始窗口配置保持一致。
#[tauri::command]
fn set_widget_layout(
    app: tauri::AppHandle,
    expanded: bool,
    menu_height: u32,
    tucked: bool,
) -> Result<(), String> {
    let window = app.get_webview_window("widget").ok_or("悬浮窗不存在。")?;
    let state = app.state::<AppState>();
    let (left, top, right, bottom) = work_area(&window)?;
    let scale = window.scale_factor().unwrap_or(1.0);
    let position = window.outer_position().map_err(|_| "无法读取窗口位置。")?;
    let (side, anchor, hide) = {
        let mut layout = lock(&state.layout)?;
        if !layout.tucked {
            layout.anchor = Some((position.x, position.y));
        }
        layout.expanded = expanded;
        layout.tucked = tucked && layout.side.is_some();
        (
            layout.side.clone(),
            layout.anchor.unwrap_or((position.x, position.y)),
            layout.tucked,
        )
    };
    let (width, height) = if hide {
        if side.as_deref() == Some("top") {
            (48, 28)
        } else {
            (28, 48)
        }
    } else {
        (
            360,
            if expanded {
                540
            } else {
                124_u32.max(86 + menu_height.min(208))
            },
        )
    };
    window
        .set_size(tauri::LogicalSize::new(width, height))
        .map_err(|_| "无法调整窗口尺寸。")?;
    let physical_width = (width as f64 * scale).round() as i32;
    let physical_height = (height as f64 * scale).round() as i32;
    let margin = 0;
    let x = match side.as_deref() {
        Some("left") => left + margin,
        Some("right") => right - physical_width - margin,
        Some("top") if hide => anchor.0 + (360.0 * scale / 2.0).round() as i32 - physical_width / 2,
        _ => anchor.0,
    }
    .clamp(left, (right - physical_width).max(left));
    let y = if side.as_deref() == Some("top") {
        top + margin
    } else {
        anchor.1
    }
    .clamp(top, (bottom - physical_height).max(top));
    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|_| "无法恢复窗口位置。")?;
    if !hide {
        snap_widget(app.clone())?;
    }
    app.emit("widget-dock", DockNotice { side, tucked: hide })
        .map_err(|_| "无法刷新边缘状态。")?;
    Ok(())
}

/// 只有用户从拖动区域开始的移动才触发自动收回；程序调整位置不会误触发。
#[tauri::command]
fn prepare_widget_drag(app: tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut layout = lock(&state.layout)?;
    layout.side = None;
    state.dragging.store(true, Ordering::Relaxed);
    app.emit(
        "widget-dock",
        DockNotice {
            side: None,
            tucked: false,
        },
    )
    .map_err(|_| "无法准备拖动。".into())
}

/// 松开鼠标后识别左、右、顶部，关闭吸附时不产生边缘图标。
fn finish_widget_drag(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    if !state.dragging.swap(false, Ordering::Relaxed) {
        return Ok(());
    }
    let notice = get_widget_dock(app.clone())?;
    app.emit("widget-dock", notice)
        .map_err(|_| "无法刷新边缘状态。".into())
}

/// 首次订阅时主动读取真实边缘状态，修复启动后尚未拖动的顶部面板不会自动收起。
/// 已缩成图标或正在拖动时保留布局状态，不把小图标坐标覆盖为面板锚点。
#[tauri::command]
fn get_widget_dock(app: tauri::AppHandle) -> Result<DockNotice, String> {
    let state = app.state::<AppState>();
    {
        let layout = lock(&state.layout)?;
        if layout.tucked || state.dragging.load(Ordering::Relaxed) {
            return Ok(DockNotice {
                side: layout.side.clone(),
                tucked: layout.tucked,
            });
        }
    }
    let window = app.get_webview_window("widget").ok_or("悬浮窗不存在。")?;
    let position = window.outer_position().map_err(|_| "无法读取位置。")?;
    let size = window.outer_size().map_err(|_| "无法读取尺寸。")?;
    let area = work_area(&window)?;
    let margin = (10.0 * window.scale_factor().unwrap_or(1.0)) as i32;
    let enabled = lock(&state.store)?.config.preferences.snap_to_edge;
    let side = widget_dock_side(enabled, (position.x, position.y), size.width, area, margin);
    let mut layout = lock(&state.layout)?;
    layout.side = side.map(String::from);
    layout.anchor = Some((position.x, position.y));
    Ok(DockNotice {
        side: layout.side.clone(),
        tucked: false,
    })
}

/// 首次打开和拖动结束共用边缘判定；坐标/宽度为物理像素，margin 已换算 DPI。
fn widget_dock_side(
    enabled: bool,
    position: (i32, i32),
    width: u32,
    area: (i32, i32, i32, i32),
    margin: i32,
) -> Option<&'static str> {
    let (left, top, right, _) = area;
    if !enabled {
        None
    } else if position.0 <= left + margin + 1 {
        Some("left")
    } else if position.0 + width as i32 >= right - margin - 1 {
        Some("right")
    } else if position.1 <= top + margin + 1 {
        Some("top")
    } else {
        None
    }
}

/// 定时器属于桌面后端，边签与托盘隐藏时仍运行；互斥门禁保证不会重叠请求。
fn start_scheduler(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let due = scheduled_ready(&app.state::<AppState>(), Instant::now());
            if due == Ok(true) {
                // 与手动入口共用鉴权、规则快照、超时和历史，不自动重试失败请求。
                let _ = run_test(app.clone(), app.state::<AppState>(), true).await;
            }
        }
    });
}

/// 轮询只预检，不提前消费期限；真正领取在运行门禁内再次验证，避免手动重置后误执行。
fn scheduled_ready(state: &AppState, now: Instant) -> Result<bool, String> {
    let running = lock(&state.running)?;
    if running.is_some() {
        return Ok(false);
    }
    let store = lock(&state.store)?;
    if !store.config.preferences.auto_test_enabled
        || !store
            .config
            .profiles
            .iter()
            .any(|p| Some(&p.id) == store.config.selected_profile_id.as_ref())
    {
        return Ok(false);
    }
    Ok(now >= *lock(&state.next_auto_test)?)
}

/// 领取测试时同步更新下次期限。手动调用始终重新计时，自动调用仅在启用且到期时领取。
/// 调用方须先持有运行门禁和配置锁；`now` 为单调时钟，间隔单位为分钟，边界 1—1440。
fn claim_test_deadline(
    preferences: &Preferences,
    next: &mut Instant,
    now: Instant,
    automatic: bool,
) -> bool {
    if automatic && (!preferences.auto_test_enabled || now < *next) {
        return false;
    }
    *next = now
        + std::time::Duration::from_secs(
            preferences.auto_test_interval_minutes.clamp(1, 1440) * 60,
        );
    true
}

/// 快捷键与托盘共用显示逻辑，恢复时夹紧工作区，解决显示器拔出后的不可见窗口。
fn show_widget(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("widget") {
        let expanded = app
            .state::<AppState>()
            .layout
            .lock()
            .map(|layout| layout.expanded)
            .unwrap_or(false);
        let _ = set_widget_layout(app.clone(), expanded, 0, false);
        let _ = snap_widget(app.clone());
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// 构建本地桌面应用；single-instance 是首个插件，第二次启动仅恢复现有窗口。
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_widget(app)
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    if let Some(window) = app.get_webview_window("widget") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            show_widget(app);
                        }
                    }
                })
                .build(),
        )
        .setup(|app| {
            let folder = app.path().app_data_dir()?;
            std::fs::create_dir_all(&folder)?;
            let store = Store::open(&folder.join("probe.db")).map_err(std::io::Error::other)?;
            let position = store.config.widget_position;
            let mut preferences = store.config.preferences.clone();
            app.manage(AppState {
                store: Mutex::new(store),
                running: Mutex::new(None),
                move_generation: AtomicU64::new(0),
                settings_gate: Mutex::new(()),
                layout: Mutex::new(WidgetLayout::default()),
                dragging: AtomicBool::new(false),
                next_auto_test: Mutex::new(
                    Instant::now()
                        + std::time::Duration::from_secs(
                            preferences.auto_test_interval_minutes * 60,
                        ),
                ),
            });
            let handle = app.handle();
            if apply_preferences(handle, &preferences).is_err() {
                // 快捷键被占用不阻止应用启动；保存关闭状态，用户可在设置中重新启用并查看具体错误。
                preferences.shortcut_enabled = false;
                let _ = apply_preferences(handle, &preferences);
                let state = app.state::<AppState>();
                let mut store = lock(&state.store).map_err(std::io::Error::other)?;
                let mut config = store.config.clone();
                config.preferences = preferences;
                store.save_config(config).map_err(std::io::Error::other)?;
            }
            let window = app
                .get_webview_window("widget")
                .ok_or("悬浮窗初始化失败。")?;
            if let Some((x, y)) = position {
                window.set_position(PhysicalPosition::new(x, y))?;
            } else {
                let (_, top, right, _) = work_area(&window).map_err(std::io::Error::other)?;
                let scale = window.scale_factor()?;
                window.set_position(PhysicalPosition::new(
                    // 初次显示在工作区右侧留 22 逻辑像素，与 360px 面板宽度一起换算 DPI。
                    right - (382.0 * scale) as i32,
                    top + (90.0 * scale) as i32,
                ))?;
            }
            snap_widget(handle.clone()).map_err(std::io::Error::other)?;
            use tauri::{
                menu::{Menu, MenuItem},
                tray::TrayIconBuilder,
            };
            let show = MenuItem::with_id(app, "show", "显示悬浮组件", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出测智雷达", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &settings, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .tooltip("测智雷达")
                .menu(&menu)
                .show_menu_on_left_click(true);
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.on_menu_event(|app, event| match event.id.as_ref() {
                "show" => show_widget(app),
                "settings" => {
                    let handle = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = open_settings(handle).await;
                    });
                }
                "quit" => {
                    // 退出立即取消本地等待，不承诺服务商撤销计费；进程关闭后数据库连接自动释放。
                    if let Ok(running) = app.state::<AppState>().running.lock() {
                        if let Some(run) = running.as_ref() {
                            run.token.cancel();
                        }
                    }
                    app.exit(0);
                }
                _ => (),
            })
            .build(app)?;
            window.show()?;
            start_scheduler(handle.clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "widget" {
                return;
            }
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                tauri::WindowEvent::Moved(_) => {
                    // 拖动结束 220ms 后再吸附和持久化，避免每个鼠标移动都执行写盘。
                    let app = window.app_handle().clone();
                    let state = app.state::<AppState>();
                    let generation = state.move_generation.fetch_add(1, Ordering::Relaxed) + 1;
                    tauri::async_runtime::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(220)).await;
                        #[cfg(windows)]
                        loop {
                            // 原生拖动会接管鼠标，等待真正松开，防止暂停拖动时突然缩成图标。
                            let pressed = unsafe {
                                windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(1)
                            } < 0;
                            if !pressed {
                                break;
                            }
                            if app
                                .state::<AppState>()
                                .move_generation
                                .load(Ordering::Relaxed)
                                != generation
                            {
                                return;
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        }
                        if app
                            .state::<AppState>()
                            .move_generation
                            .load(Ordering::Relaxed)
                            == generation
                        {
                            let _ = snap_widget(app.clone());
                            let _ = finish_widget_drag(&app);
                        }
                    });
                }
                _ => (),
            }
        })
        .invoke_handler(tauri::generate_handler![
            autostart::get_autostart,
            autostart::set_autostart,
            updates::open_external_url,
            updates::check_updates,
            fetch_models,
            test_connection,
            get_snapshot,
            get_widget_dock,
            save_profile,
            delete_profile,
            select_profile,
            save_rule,
            save_preferences,
            start_test,
            cancel_test,
            clear_history,
            open_settings,
            snap_widget,
            set_widget_layout,
            prepare_widget_drag
        ])
        .run(tauri::generate_context!())
        .expect("测智雷达启动失败，请检查本地数据目录和系统环境。");
}

#[cfg(test)]
mod desktop_tests {
    use super::*;

    /// 拖动结束一次贴边，无10px过渡位置；125%物理尺寸和负坐标屏幕也保持一致。
    #[test]
    fn snapping_has_no_intermediate_margin() {
        assert_eq!(
            snapped_position((1551, 100), (360, 540), (0, 0, 1920, 1040), true, 16),
            (1560, 100)
        );
        assert_eq!(
            snapped_position((9, 4), (360, 124), (0, 0, 1920, 1040), true, 16),
            (0, 0)
        );
        assert_eq!(
            snapped_position((-460, -193), (450, 675), (-1920, -200, 0, 1000), true, 20),
            (-450, -200)
        );
        assert_eq!(
            snapped_position((9, 100), (360, 124), (0, 0, 1920, 1040), false, 16),
            (9, 100)
        );
    }

    /// 启动与拖动采用同一规则；覆盖负坐标副屏、关闭吸附和角落侧边优先级。
    #[test]
    fn detects_initial_edges_and_negative_monitor_coordinates() {
        let area = (0, 0, 1920, 1040);
        assert_eq!(
            widget_dock_side(true, (700, 10), 360, area, 10),
            Some("top")
        );
        assert_eq!(
            widget_dock_side(true, (10, 400), 360, area, 10),
            Some("left")
        );
        assert_eq!(
            widget_dock_side(true, (1550, 400), 360, area, 10),
            Some("right")
        );
        assert_eq!(
            widget_dock_side(true, (10, 10), 360, area, 10),
            Some("left")
        );
        assert_eq!(widget_dock_side(true, (700, 100), 360, area, 10), None);
        assert_eq!(widget_dock_side(false, (700, 10), 360, area, 10), None);
        assert_eq!(
            widget_dock_side(true, (-1000, -190), 360, (-1920, -200, 0, 840), 10),
            Some("top")
        );
    }

    /// 使用真实门禁和模拟单调时间，不需要等待三分钟或调用模型。
    #[test]
    fn schedule_waits_deduplicates_and_skips_disabled_or_busy() {
        let now = Instant::now();
        let state = AppState {
            store: Mutex::new(Store::open(std::path::Path::new(":memory:")).unwrap()),
            running: Mutex::new(None),
            move_generation: AtomicU64::new(0),
            settings_gate: Mutex::new(()),
            layout: Mutex::new(WidgetLayout::default()),
            dragging: AtomicBool::new(false),
            next_auto_test: Mutex::new(now),
        };
        assert!(!scheduled_ready(&state, now).unwrap());
        {
            let mut store = state.store.lock().unwrap();
            store.config.profiles.push(ModelProfile {
                id: "one".into(),
                name: "测试模型".into(),
                base_url: "http://localhost/v1".into(),
                model: "fixture".into(),
                has_key: false,
            });
            store.config.selected_profile_id = Some("one".into());
        }
        assert!(scheduled_ready(&state, now).unwrap());
        {
            let preferences = state.store.lock().unwrap().config.preferences.clone();
            assert!(claim_test_deadline(
                &preferences,
                &mut state.next_auto_test.lock().unwrap(),
                now,
                true,
            ));
        }
        assert!(!scheduled_ready(&state, now).unwrap());
        assert!(!scheduled_ready(&state, now + std::time::Duration::from_secs(179)).unwrap());
        assert!(scheduled_ready(&state, now + std::time::Duration::from_secs(180)).unwrap());
        state
            .store
            .lock()
            .unwrap()
            .config
            .preferences
            .auto_test_enabled = false;
        assert!(!scheduled_ready(&state, now + std::time::Duration::from_secs(400)).unwrap());
        state
            .store
            .lock()
            .unwrap()
            .config
            .preferences
            .auto_test_enabled = true;
        *state.running.lock().unwrap() = Some(Running {
            info: ActiveRun {
                id: "run".into(),
                profile_name: "测试模型".into(),
                model: "fixture".into(),
                started_at: "2026-10-23T00:00:00Z".into(),
            },
            token: CancellationToken::new(),
        });
        assert!(!scheduled_ready(&state, now + std::time::Duration::from_secs(400)).unwrap());
    }

    /// 剩余两分钟时手动测试必须重新等待三分钟；已通过预检的自动轮询也不能抢跑。
    #[test]
    fn manual_test_restarts_full_interval_and_invalidates_old_auto_deadline() {
        let origin = Instant::now();
        let preferences = Preferences::default();
        let at = |seconds| origin + std::time::Duration::from_secs(seconds);
        let mut deadline = at(180);
        assert!(claim_test_deadline(
            &preferences,
            &mut deadline,
            at(60),
            false
        ));
        assert_eq!(deadline, at(240));
        assert!(!claim_test_deadline(
            &preferences,
            &mut deadline,
            at(180),
            true
        ));
        assert!(!claim_test_deadline(
            &preferences,
            &mut deadline,
            at(239),
            true
        ));
        assert!(claim_test_deadline(
            &preferences,
            &mut deadline,
            at(240),
            true
        ));
        assert!(!claim_test_deadline(
            &preferences,
            &mut deadline,
            at(240),
            true
        ));

        // 模拟轮询已在到期时完成预检，但手动请求先获得运行门禁并重置期限。
        assert!(claim_test_deadline(
            &preferences,
            &mut deadline,
            at(420),
            false
        ));
        assert!(!claim_test_deadline(
            &preferences,
            &mut deadline,
            at(420),
            true
        ));
        assert_eq!(deadline, at(600));
    }

    /// 老配置补齐新增字段，同时保留原有关闭选项。
    #[test]
    fn old_preferences_keep_values_and_get_schedule_defaults() {
        let preferences: Preferences = serde_json::from_value(serde_json::json!({"alwaysOnTop":false,"snapToEdge":false,"opacity":95,"timeoutSeconds":30,"shortcutEnabled":false})).unwrap();
        assert!(!preferences.always_on_top);
        assert!(preferences.auto_test_enabled);
        assert_eq!(preferences.auto_test_interval_minutes, 3);
    }
}
