//! 桌面生命周期与 IPC。所有变更按 running → store 的固定锁顺序执行；
//! 网络等待不持有互斥锁，应用内一次只能有一个付费测试请求。作者：ahui。

pub mod api;
mod autostart;
pub mod backup;
mod chat_sse;
pub mod conditions;
mod discovery;
mod html;
pub mod model;
pub mod notifications;
mod pet_window;
pub mod preview;
mod response_diagnostics;
mod responses_sse;
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
    Emitter, Listener, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const CREDENTIAL_SERVICE: &str = "com.ahui.model-probe";

/// 拖动世代切换与最终位置保存共用短门禁；不跨窗口API或事件发送持锁，避免同步回调死锁。
static DRAG_STATE_GATE: Mutex<()> = Mutex::new(());

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
    /// 原生拖动从按下到移动之间可能没有 `Moved` 事件；单独记录是否真的移动，
    /// 避免仅点击标题栏就清除边缘吸附状态，导致鼠标离开后无法自动收起。
    drag_moved: AtomicBool,
    /// 每次按下独立世代，旧释放监视器不能结束后续拖动。
    drag_generation: AtomicU64,
    drag_origin: Mutex<Option<(i32, i32)>>,
    next_auto_test: Mutex<Instant>,
    notifications: Mutex<notifications::NotificationTracker>,
    notification_error: Mutex<Option<String>>,
    /// 清空相关分类后递增；旧 SMTP 任务仍可收尾，但不得把过期故障提示写回新状态。
    notification_generation: AtomicU64,
}

/// 仅保存本次进程的边缘呈现状态；数据库仍保存完整面板位置，重启可直接操作。
#[derive(Default)]
struct WidgetLayout {
    side: Option<String>,
    tucked: bool,
    expanded: bool,
    anchor: Option<(i32, i32)>,
    /// 页内放大临时借用当前宿主尺寸，关闭时恢复；不能持久化为正常卡片位置。
    preview_restore: Option<((i32, i32), (u32, u32))>,
}

/// 事件不携带配置或凭据；前端据此切换边缘图标并显示最新测试颜色。
#[derive(Clone, serde::Serialize)]
struct DockNotice {
    side: Option<String>,
    tucked: bool,
}

/// 配置事务已提交后的附加提示；warning 不表示保存失败，调用方不得因此重复写入。
#[derive(serde::Serialize)]
struct SaveOutcome {
    warning: Option<String>,
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

/// 凭据可决定同网关请求的账户/上游，因此实际替换或移除也属于测试条件变化。
/// 只保存随机修订标识，不对密钥做可离线猜测的散列；重复保存相同密钥保持结论。
/// has_key=false 时忽略系统残留条目，与请求实际不读取该凭据的边界保持一致。
fn next_credential_revision(
    previous: Option<&ModelProfile>,
    old_key: Option<&str>,
    next_key: Option<&str>,
) -> Option<String> {
    let had_key = previous.is_some_and(|profile| profile.has_key);
    let effective_key = if had_key { old_key } else { None };
    if had_key != next_key.is_some() || effective_key != next_key {
        Some(Uuid::new_v4().to_string())
    } else {
        previous.and_then(|profile| profile.credential_revision.clone())
    }
}

#[tauri::command]
fn get_snapshot(
    state: State<'_, AppState>,
    include_history: Option<bool>,
) -> Result<AppSnapshot, String> {
    let running = lock(&state.running)?;
    let store = lock(&state.store)?;
    // 悬浮窗优先读取当前条件匹配的一条最近记录；无匹配项时保留分组最近时间并提示重测。
    // 设置页改用独立摘要/正文分页；仅明确导出或旧调用方才显式请求完整历史，
    // 避免自动测试每次广播时反复序列化最多 128 MiB 的原始回答。
    let condition_fingerprints: std::collections::HashMap<_, _> = store
        .config
        .profiles
        .iter()
        .filter_map(|profile| {
            // 单个历史配置不再合法时只撤销其结论，不阻断设置页读取和修复。
            conditions::condition_fingerprint(&store.config, profile)
                .ok()
                .map(|fingerprint| (profile.id.clone(), fingerprint))
        })
        .collect();
    let history = if include_history.unwrap_or(false) {
        store.history()?
    } else {
        store
            .config
            .selected_profile_id
            .as_deref()
            .map(|profile_id| {
                store.latest_record_for_conditions(
                    profile_id,
                    condition_fingerprints.get(profile_id).map(String::as_str),
                )
            })
            .transpose()?
            .into_iter()
            .flatten()
            .collect()
    };
    Ok(AppSnapshot {
        config: store.config.clone(),
        condition_fingerprints,
        history,
        history_revision: store.history_revision()?,
        email_notification_error: lock(&state.notification_error)?.clone(),
        running: running.as_ref().map(|run| run.info.clone()),
        auto_test_remaining_ms: if running.is_none()
            && store.config.preferences.auto_test_enabled
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

/// 历史索引在独立只读事务/后台线程加载，避免扫描长回答时持有应用存储锁。
#[tauri::command]
async fn get_history_index(app: tauri::AppHandle) -> Result<storage::HistoryIndex, String> {
    tokio::task::spawn_blocking(move || {
        let reader = lock(&app.state::<AppState>().store)?.history_reader()?;
        reader.history_index()
    })
    .await
    .map_err(|_| "历史索引任务失败。".to_string())?
}

/// 只允许每页最多十条；正文用于当前页预览、详情、复制，不改写历史或重新判定。
#[tauri::command]
async fn get_history_records(
    app: tauri::AppHandle,
    sequences: Vec<i64>,
) -> Result<Vec<TestRecord>, String> {
    tokio::task::spawn_blocking(move || {
        let reader = lock(&app.state::<AppState>().store)?.history_reader()?;
        reader.history_records(&sequences)
    })
    .await
    .map_err(|_| "历史页面任务失败。".to_string())?
}

/// 只返回全文命中的稳定记录 ID；输入防抖由界面负责，后端不把搜索当作接口请求。
#[tauri::command]
async fn search_history(app: tauri::AppHandle, query: String) -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(move || {
        let reader = lock(&app.state::<AppState>().store)?.history_reader()?;
        reader.search_history(&query)
    })
    .await
    .map_err(|_| "历史搜索任务失败。".to_string())?
}

/// 临时草稿诊断只读凭据，不保存表单；改域名时不得隐式发送旧配置的密钥。
/// 凭据与配置版本在同一存储锁内捕获，防止旧凭据请求结果关联到并发保存的新配置。
fn diagnostic_context(
    state: &AppState,
    input: &ProfileInput,
) -> Result<(String, Option<ModelProfile>), String> {
    if !matches!(
        input.interface_mode.as_str(),
        "chat_completions" | "responses"
    ) {
        return Err("接口模式必须是 Chat Completions 或 Responses。".into());
    }
    if input.interface_mode == "responses" {
        api::responses_endpoint(&input.base_url)?;
    } else {
        api::endpoint(&input.base_url)?;
    }
    validate_reasoning_effort(input.reasoning_effort.as_deref())?;
    if input.api_key.len() > 2500 || input.api_key.contains(['\r', '\n']) {
        return Err("API Key 太长或包含换行。".into());
    }
    if input.remove_key && !input.api_key.is_empty() {
        return Err("移除密钥与输入新密钥不能同时使用。".into());
    }
    let store = lock(&state.store)?;
    let original = store
        .config
        .profiles
        .iter()
        .find(|p| p.id == input.id)
        .cloned();
    if !input.api_key.is_empty() {
        return Ok((input.api_key.clone(), original));
    }
    if input.remove_key || input.id.is_empty() {
        return Ok((String::new(), original));
    }
    let saved = original.as_ref().ok_or("原配置不存在，请重新选择。")?;
    // 同一服务只切换协议无需重输密钥；统一成 Chat 端点比较，仍拒绝改域名或基础路径。
    let original_endpoint = api::endpoint(&saved.base_url)?;
    let input_endpoint = api::endpoint(&input.base_url)?;
    if saved.has_key && original_endpoint != input_endpoint {
        return Err("接口地址已修改，请明确输入用于新地址的密钥后重试。".into());
    }
    let key = if saved.has_key {
        read_key(&credential(&input.id)?)?.ok_or("原配置密钥不存在，请重新输入。")?
    } else {
        String::new()
    };
    Ok((key, original))
}

/// 拉取模型仅发 GET，不消耗测智周期，不写入历史；诊断门禁跨窗口去重。
#[tauri::command]
async fn fetch_models(
    state: State<'_, AppState>,
    input: ProfileInput,
) -> Result<discovery::ModelCatalog, String> {
    let _guard = discovery::DiagnosticGuard::acquire()?;
    let (key, original) = diagnostic_context(&state, &input)?;
    let endpoint = discovery::models_endpoint(&input.base_url)?.to_string();
    let unchanged = original.as_ref().is_some_and(|item| {
        input.api_key.is_empty()
            && !input.remove_key
            && discovery::models_endpoint(&item.base_url).is_ok_and(|url| url.as_str() == endpoint)
    });
    let token = if unchanged {
        original
            .as_ref()
            .and_then(|item| item.catalog_token.clone())
    } else {
        None
    }
    .unwrap_or_else(|| Uuid::new_v4().to_string());
    let models = discovery::fetch_models(&input.base_url, &key).await?;
    let mut store = lock(&state.store)?;
    store.save_model_catalog(&token, &endpoint, &models)?;
    // 只关联拉取开始/结束时地址与凭据版本均未改变的已保存配置；未保存草稿随保存关联。
    if unchanged {
        let mut config = store.config.clone();
        if let Some(current) = config.profiles.iter_mut().find(|item| item.id == input.id) {
            if original.as_ref().is_some_and(|before| {
                current.credential_revision == before.credential_revision
                    && current.base_url == before.base_url
                    && current.catalog_token == before.catalog_token
            }) && current.catalog_token.as_deref() != Some(token.as_str())
            {
                current.catalog_token = Some(token.clone());
                store.save_config(config)?;
            }
        }
    }
    Ok(discovery::ModelCatalog { token, models })
}

/// 缓存只读取同一端点，无网络及凭据读取；输入表单的密钥不进入缓存查询或日志。
#[tauri::command]
fn get_model_catalog(
    state: State<'_, AppState>,
    token: String,
    base_url: String,
) -> Result<Vec<discovery::FetchedModel>, String> {
    let endpoint = discovery::models_endpoint(&base_url)?.to_string();
    lock(&state.store)?.model_catalog(&token, &endpoint)
}

/// 悬浮窗只提交已保存分组 ID，凭据仍在后端读取，不接受前端拼装的主机/密钥快照。
/// 复用诊断门禁和持久缓存，不调用推理或自动换模型；迟到响应由前端世代隔离。
#[tauri::command]
async fn fetch_profile_models(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<discovery::ModelCatalog, String> {
    let input = {
        let store = lock(&state.store)?;
        let profile = store
            .config
            .profiles
            .iter()
            .find(|item| item.id == profile_id)
            .ok_or("模型配置已不存在，请刷新后重试。")?;
        ProfileInput {
            id: profile.id.clone(),
            name: profile.name.clone(),
            base_url: profile.base_url.clone(),
            model: profile.model.clone(),
            api_key: String::new(),
            remove_key: false,
            interface_mode: profile.interface_mode.clone(),
            protocol_fallback_enabled: profile.protocol_fallback_enabled,
            reasoning_effort: profile.reasoning_effort.clone(),
            rule_preset_id: profile.rule_preset_id.clone(),
            catalog_token: profile.catalog_token.clone(),
        }
    };
    let catalog = fetch_models(state, input).await?;
    changed(&app);
    Ok(catalog)
}

/// 快捷换模型仅写指定分组的 model 字段；校验当前目录与运行门禁，保留凭据及其他设置。
#[tauri::command]
fn set_profile_model(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    profile_id: String,
    model: String,
) -> Result<(), String> {
    let _idle = require_idle(&state)?;
    lock(&state.store)?.set_profile_model(&profile_id, &model)?;
    changed(&app);
    Ok(())
}

/// 独立固定提示词连接测试，按草稿绑定的已保存规则/全局值确定超时，不读取旧接口超时。
#[tauri::command]
async fn test_connection(
    state: State<'_, AppState>,
    input: ProfileInput,
) -> Result<discovery::ConnectionResult, String> {
    let _guard = discovery::DiagnosticGuard::acquire()?;
    let (key, _) = diagnostic_context(&state, &input)?;
    // 请求开始时取同一份持久化超时；不在网络等待期间持有存储锁。
    let timeout = {
        let store = lock(&state.store)?;
        conditions::rule_for_binding(&store.config, input.rule_preset_id.as_deref())?
            .timeout_seconds
            .unwrap_or(store.config.preferences.timeout_seconds)
    };
    discovery::test_connection(
        &input.base_url,
        &input.model,
        &key,
        timeout,
        &input.interface_mode,
        input.reasoning_effort.clone(),
        input.protocol_fallback_enabled,
    )
    .await
}

/// 保存接口：凭据操作先行；若数据库提交失败，尝试恢复原凭据并明确报告回滚失败。
#[tauri::command]
fn save_profile(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: ProfileInput,
) -> Result<(), String> {
    let _idle = require_idle(&state)?;
    if !matches!(
        input.interface_mode.as_str(),
        "chat_completions" | "responses"
    ) {
        return Err("接口模式必须是 Chat Completions 或 Responses。".into());
    }
    if input.name.trim().is_empty()
        || input.name.chars().count() > 80
        || input.model.trim().is_empty()
        || input.model.chars().count() > 200
        || input.base_url.len() > 2000
    {
        return Err("配置名称、模型名称或地址不符合长度要求。".into());
    }
    if input.interface_mode == "responses" {
        api::responses_endpoint(&input.base_url)?;
    } else {
        api::endpoint(&input.base_url)?;
    }
    validate_reasoning_effort(input.reasoning_effort.as_deref())?;
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
    if let Some(previous) = config.profiles.iter().find(|item| item.id == id) {
        if previous.has_key
            && !input.remove_key
            && input.api_key.is_empty()
            && api::endpoint(&previous.base_url)? != api::endpoint(&input.base_url)?
        {
            return Err("接口地址已修改，请输入新地址使用的密钥，或明确移除已有密钥。".into());
        }
    }
    let next_key = if input.remove_key {
        None
    } else if input.api_key.is_empty() {
        // 导入配置的 has_key=false 是明确边界，不能因系统残留同 ID 凭据而恢复旧密钥。
        config
            .profiles
            .iter()
            .find(|profile| profile.id == id && profile.has_key)
            .and(old_key.clone())
    } else {
        Some(input.api_key)
    };
    let previous = config.profiles.iter().find(|item| item.id == id);
    let mut catalog_token = input.catalog_token;
    // 旧句柄不能跨凭据或地址复用；新拉取的句柄还须通过缓存端点检查。
    if previous.is_some_and(|item| {
        item.catalog_token == catalog_token
            && (old_key != next_key
                || discovery::models_endpoint(&item.base_url).ok()
                    != discovery::models_endpoint(&input.base_url).ok())
    }) {
        catalog_token = None;
    }
    if let Some(token) = &catalog_token {
        if store
            .model_catalog(token, discovery::models_endpoint(&input.base_url)?.as_str())?
            .is_empty()
        {
            catalog_token = None;
        }
    }
    let profile = ModelProfile {
        id: id.clone(),
        name: input.name.trim().into(),
        base_url: input.base_url.trim().trim_end_matches('/').into(),
        model: input.model.trim().into(),
        has_key: next_key.is_some(),
        credential_revision: next_credential_revision(
            config.profiles.iter().find(|item| item.id == id),
            old_key.as_deref(),
            next_key.as_deref(),
        ),
        interface_mode: input.interface_mode,
        protocol_fallback_enabled: input.protocol_fallback_enabled,
        reasoning_effort: input.reasoning_effort,
        timeout_seconds: None,
        rule_preset_id: input.rule_preset_id,
        catalog_token,
    };
    if let Some(previous) = config.profiles.iter_mut().find(|item| item.id == id) {
        *previous = profile;
    } else {
        // 首版合法旧库可能已超过 100 个：允许编辑现有项，但不能继续新增。
        if config.profiles.len() >= 100 {
            return Err("模型配置最多新增至 100 个；已有超额旧配置可继续编辑或删除。".into());
        }
        config.profiles.push(profile);
    }
    config.selected_profile_id = Some(id);
    conditions::validate_stored_config(&config)?;
    set_key(&entry, next_key.as_deref())?;
    if let Err(message) = store.save_config(config) {
        if set_key(&entry, old_key.as_deref()).is_err() {
            return Err(format!("{message} 凭据回滚也失败，请重新保存该配置。"));
        }
        return Err(message);
    }
    drop(store);
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
    drop(store);
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
    drop(store);
    changed(&app);
    Ok(())
}

/// 默认规则名称与题答在一个配置事务中保存；旧调用省略名称时保留已有名称。
#[tauri::command]
fn save_rule(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    rule: TestRule,
    name: Option<String>,
) -> Result<(), String> {
    let _idle = require_idle(&state)?;
    rule.validate()?;
    let mut store = lock(&state.store)?;
    let mut config = store.config.clone();
    config.rule = rule;
    if let Some(name) = name {
        if name.trim().is_empty() || name.chars().count() > 80 {
            return Err("规则名称必须为 1—80 个字符。".into());
        }
        config.default_rule_name = name.trim().to_owned();
    }
    store.save_config(config)?;
    drop(store);
    changed(&app);
    Ok(())
}

/// 保存或更新规则预设；每个预设沿用与全局规则相同的长度和空白校验。
#[tauri::command]
fn save_rule_preset(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    preset: RulePreset,
) -> Result<(), String> {
    let _idle = require_idle(&state)?;
    conditions::validate_id(&preset.id)?;
    if preset.name.trim().is_empty() || preset.name.chars().count() > 80 {
        return Err("预设名称必须为 1—80 个字符。".into());
    }
    let rule = TestRule {
        prompt: preset.prompt.clone(),
        keyword: preset.keyword.clone(),
        alternative_keywords: preset.alternative_keywords.clone(),
        timeout_seconds: preset.timeout_seconds,
    };
    rule.validate()?;
    let mut store = lock(&state.store)?;
    let mut config = store.config.clone();
    if let Some(existing) = config
        .rule_presets
        .iter_mut()
        .find(|item| item.id == preset.id)
    {
        *existing = preset;
    } else {
        if !is_builtin_rule_id(&preset.id)
            && config
                .rule_presets
                .iter()
                .filter(|item| !is_builtin_rule_id(&item.id))
                .count()
                >= 100
        {
            return Err("自定义规则预设最多保存 100 组。".into());
        }
        config.rule_presets.push(preset);
    }
    store.save_config(config)?;
    drop(store);
    changed(&app);
    Ok(())
}

/// 卡片只修改指定模型的规则绑定，避免提交整个模型而覆盖设置页草稿或凭据。
/// 与正式测试共用运行门禁，持锁核验当前对象和规则；不重置自动倒计时，不自动请求。
#[tauri::command]
fn set_profile_rule(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    profile_id: String,
    rule_preset_id: Option<String>,
) -> Result<(), String> {
    let _idle = require_idle(&state)?;
    let mut store = lock(&state.store)?;
    let mut config = store.config.clone();
    let index = config
        .profiles
        .iter()
        .position(|item| item.id == profile_id)
        .ok_or("模型配置已不存在，请刷新后重试。")?;
    config.profiles[index].rule_preset_id = rule_preset_id;
    conditions::effective_rule(&config, &config.profiles[index])?;
    store.save_config(config)?;
    drop(store);
    changed(&app);
    Ok(())
}

/// 快捷切换思考等级仅修改指定字段；运行门禁、持久化与结果指纹沿用正式配置路径。
/// 不触碰凭据、规则绑定或自动测试期限；null 明确表示省略请求参数，非法档位原样拒绝。
#[tauri::command]
fn set_profile_effort(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    profile_id: String,
    reasoning_effort: Option<String>,
) -> Result<(), String> {
    validate_reasoning_effort(reasoning_effort.as_deref())?;
    let _idle = require_idle(&state)?;
    let mut store = lock(&state.store)?;
    let mut config = store.config.clone();
    let profile = config
        .profiles
        .iter_mut()
        .find(|item| item.id == profile_id)
        .ok_or("模型配置已不存在，请刷新后重试。")?;
    profile.reasoning_effort = reasoning_effort;
    store.save_config(config)?;
    drop(store);
    changed(&app);
    Ok(())
}

/// 删除预设；被引用时必须明确同意原子解绑，避免静默改变模型测试题目。
#[tauri::command]
fn delete_rule_preset(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    unbind_profiles: Option<bool>,
) -> Result<(), String> {
    let _idle = require_idle(&state)?;
    let mut store = lock(&state.store)?;
    let mut config = store.config.clone();
    if !unbind_profiles.unwrap_or(false)
        && config
            .profiles
            .iter()
            .any(|profile| profile.rule_preset_id.as_deref() == Some(id.as_str()))
    {
        return Err("该规则预设仍被模型配置使用，请先解除绑定。".into());
    }
    // 仅用户明确确认后统一解除引用；解绑和删除在同一配置事务中落盘。
    for profile in &mut config.profiles {
        if profile.rule_preset_id.as_deref() == Some(id.as_str()) {
            profile.rule_preset_id = None;
        }
    }
    let before = config.rule_presets.len();
    config.rule_presets.retain(|preset| preset.id != id);
    if before == config.rule_presets.len() {
        return Err("规则预设不存在。".into());
    }
    store.save_config(config)?;
    drop(store);
    changed(&app);
    Ok(())
}

/// 置顶变更若失败则不写数据库；数据库写入失败时恢复此前运行时设置。
/// 未明确编辑自动开关的保存，在存储锁内保留该权威字段，避免托盘已更新但
/// 跨窗口事件尚未送达时，整个旧偏好对象将外部开关覆盖；省略参数兼容旧调用方。
#[tauri::command]
fn save_preferences(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    mut preferences: Preferences,
    confirm_cleanup: Option<bool>,
    preserve_auto_test: Option<bool>,
) -> Result<SaveOutcome, String> {
    let _idle = require_idle(&state)?;
    // 保存、清理预览、启动和备份共用边界，避免只放开表单却被另一入口的旧上限拒绝。
    conditions::validate_preferences(&preferences)?;
    let mut store = lock(&state.store)?;
    let mut config = store.config.clone();
    let previous = config.preferences.clone();
    if preserve_auto_test.unwrap_or(false) {
        preferences.auto_test_enabled = previous.auto_test_enabled;
    }
    apply_preferences(&app, &preferences)?;
    config.preferences = preferences;
    if let Err(message) = store.save_config_with_retention(config, confirm_cleanup.unwrap_or(false))
    {
        if apply_preferences(&app, &previous).is_err() {
            return Err(format!(
                "{message} 运行时设置恢复失败，请重新保存悬浮设置或重启应用。"
            ));
        }
        return Err(message);
    }
    // 透明度、置顶等展示设置不应推迟既有自动测试；只有开关或间隔
    // 真正改变调度时才从当前时刻重新计算完整周期。
    if auto_schedule_changed(&previous, &store.config.preferences) {
        *lock(&state.next_auto_test)? = Instant::now()
            + std::time::Duration::from_secs(
                store.config.preferences.auto_test_interval_minutes * 60,
            );
    }
    let restore_dock = !store.config.preferences.snap_to_edge;
    drop(store);
    let warning = restore_dock_after_commit(&app, restore_dock);
    changed(&app);
    Ok(SaveOutcome { warning })
}

/// 关闭吸边后的原生恢复属于已提交事务的呈现收尾。操作失败只返回明确警告，
/// 仍广播权威快照；不能伪装成可重试的保存/导入失败，尤其不能再次执行历史清理。
fn restore_dock_after_commit(app: &tauri::AppHandle, restore: bool) -> Option<String> {
    if !restore {
        return None;
    }
    let result = (|| {
        let state = app.state::<AppState>();
        let expanded = lock(&state.layout)?.expanded;
        set_widget_layout(app.clone(), expanded, 0, false)?;
        lock(&state.layout)?.side = None;
        app.emit(
            "widget-dock",
            DockNotice {
                side: None,
                tucked: false,
            },
        )
        .map_err(|_| "无法刷新边缘状态。".to_string())
    })();
    result.err().map(|message| {
        format!("配置已保存，但悬浮窗退出吸边失败：{message} 请重新打开应用恢复窗口。")
    })
}

/// 只读清理预览；实际保存重新计算，避免预览到提交期间新增历史被静默删除。
#[tauri::command]
fn preview_history_retention(
    state: State<'_, AppState>,
    preferences: Preferences,
) -> Result<storage::HistoryCleanupPreview, String> {
    lock(&state.store)?.preview_history_retention(&preferences)
}

/// 备份仅序列化非敏感数据；导入通过一次配置事务提交，永不循环调用单配置保存。
#[tauri::command]
fn export_config_backup(state: State<'_, AppState>) -> Result<backup::BackupDocument, String> {
    backup::export_backup(&lock(&state.store)?.config)
}

#[tauri::command]
fn preview_config_import(
    state: State<'_, AppState>,
    backup: serde_json::Value,
) -> Result<backup::ImportPreview, String> {
    backup::preview_import(&lock(&state.store)?.config, &backup)
}

/// 先验证完整合并结果；运行时设置失败则不落盘，落盘失败则恢复运行时。
#[tauri::command]
fn import_config_backup(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    backup: serde_json::Value,
    duplicate_strategy: String,
    confirm_cleanup: Option<bool>,
) -> Result<backup::ImportReport, String> {
    let _idle = require_idle(&state)?;
    let mut store = lock(&state.store)?;
    let mut prepared = backup::prepare_import(&store.config, &backup, &duplicate_strategy)?;
    let previous = store.config.preferences.clone();
    let next = prepared.config.preferences.clone();
    apply_preferences(&app, &next)?;
    if let Err(message) =
        store.save_config_with_retention(prepared.config, confirm_cleanup.unwrap_or(false))
    {
        if apply_preferences(&app, &previous).is_err() {
            return Err(format!("{message} 运行时恢复失败，请重新保存设置或重启。"));
        }
        return Err(message);
    }
    if auto_schedule_changed(&previous, &next) {
        *lock(&state.next_auto_test)? =
            Instant::now() + std::time::Duration::from_secs(next.auto_test_interval_minutes * 60);
    }
    drop(store);
    prepared.report.warning = restore_dock_after_commit(&app, !next.snap_to_edge);
    changed(&app);
    Ok(prepared.report)
}

/// 授权码先写系统凭据；数据库失败时恢复旧凭据，不把密码放入快照或日志。
#[tauri::command]
fn save_email_settings(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: notifications::EmailSettingsInput,
) -> Result<AppConfig, String> {
    let mut store = lock(&state.store)?;
    let old_password = notifications::read_password()?;
    let next_password = notifications::validate_input(&store.config.email_notifications, &input)?;
    let mut config = store.config.clone();
    config.email_notifications = input.settings;
    config.email_notifications.has_password = next_password.is_some();
    notifications::save_password(next_password.as_deref())?;
    if let Err(message) = store.save_config(config) {
        if notifications::save_password(old_password.as_deref()).is_err() {
            return Err(format!(
                "{message} 邮箱授权码恢复失败，请重新保存邮件设置。"
            ));
        }
        return Err(message);
    }
    let config = store.config.clone();
    drop(store);
    changed(&app);
    Ok(config)
}

/// 试发使用表单草稿，既不保存，也不触发模型测试或通知状态变化。
#[tauri::command]
async fn send_test_email(
    state: State<'_, AppState>,
    input: notifications::EmailSettingsInput,
) -> Result<(), String> {
    let password = {
        let store = lock(&state.store)?;
        notifications::validate_input(&store.config.email_notifications, &input)?
            .ok_or("请填写 SMTP 授权码后发送测试邮件。")?
    };
    notifications::send_test(&input.settings, &password).await
}

/// 判断偏好保存是否真正影响自动测试期限；仅外观和置顶变化时保持原期限。
fn auto_schedule_changed(previous: &Preferences, next: &Preferences) -> bool {
    previous.auto_test_enabled != next.auto_test_enabled
        || previous.auto_test_interval_minutes != next.auto_test_interval_minutes
}

/// 悬浮窗、托盘和设置页共用的自动测试开关。
///
/// 关闭开关只阻止后续自动请求，不取消已经领取的请求；开启时从当前时刻重新
/// 计算一个完整周期，避免沿用关闭前已经过期的期限立即补跑。
#[tauri::command]
fn set_auto_test_enabled(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    let mut store = lock(&state.store)?;
    if store.config.preferences.auto_test_enabled == enabled {
        return Ok(());
    }
    let mut config = store.config.clone();
    config.preferences.auto_test_enabled = enabled;
    store.save_config(config)?;
    if enabled {
        *lock(&state.next_auto_test)? = Instant::now()
            + std::time::Duration::from_secs(
                store.config.preferences.auto_test_interval_minutes * 60,
            );
    }
    drop(store);
    changed(&app);
    Ok(())
}

/// 保存、导入和分类清空共用置顶设置；不再注册全局快捷键，避免占用冲突中断事务。
fn apply_preferences(app: &tauri::AppHandle, preferences: &Preferences) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("widget") {
        window
            .set_always_on_top(preferences.always_on_top)
            .map_err(|_| "无法更新悬浮窗置顶状态。")?;
    }
    Ok(())
}

/// 手动入口与自动入口共用生命周期；下次自动测试从本次结算完成后等待完整间隔。
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
    let (profile, rule, rule_name, timeout, fingerprint, info, token) = {
        let mut running = require_idle(&state)?;
        let store = lock(&state.store)?;
        let config = &store.config;
        let profile = config
            .profiles
            .iter()
            .find(|profile| Some(&profile.id) == config.selected_profile_id.as_ref())
            .cloned()
            .ok_or("请先添加模型接口配置。")?;
        let rule = conditions::effective_rule(config, &profile)?;
        // 名称是请求开始时的展示快照；不参与条件摘要，后续改名不能改写历史。
        let rule_name = profile
            .rule_preset_id
            .as_ref()
            .and_then(|id| config.rule_presets.iter().find(|preset| &preset.id == id))
            .map(|preset| preset.name.clone())
            .unwrap_or_else(|| config.default_rule_name.clone());
        let fingerprint = conditions::condition_fingerprint(config, &profile)?;
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
        let timeout = rule
            .timeout_seconds
            .unwrap_or(config.preferences.timeout_seconds);
        (profile, rule, rule_name, timeout, fingerprint, info, token)
    };
    changed(&app);
    let start = Instant::now();
    let key_result = if profile.has_key {
        credential(&profile.id)
            .and_then(|entry| read_key(&entry))
            .and_then(|key| key.ok_or("已保存的密钥不存在，请重新保存 API Key。".into()))
    } else {
        Ok(String::new())
    };
    // 诊断只使用本次网络读取，目录定位失败也不能阻止原测试结算；不改变协议/回退配置。
    let diagnostic_directory = app
        .path()
        .app_data_dir()
        .ok()
        .map(|directory| directory.join("response-diagnostics"));
    let completion = match key_result {
        Ok(key) => {
            api::complete_with_diagnostics(
                &profile,
                &rule,
                &key,
                timeout,
                &token,
                diagnostic_directory.as_deref(),
            )
            .await
        }
        Err(message) => api::Completion::failure("credentials", &message),
    };
    // 在协议错误返回到历史结算之间收到取消也必须优先记为取消，防止发出失败邮件。
    let (answer_type, status) = html::settle(
        &completion.answer,
        &rule,
        completion.error.is_some(),
        completion.cancelled || token.is_cancelled(),
    );
    let rule_version = rule.version().to_owned();
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
        alternative_keywords: rule.alternative_keywords,
        rule_name: Some(rule_name),
        rule_version,
        source: Some(if automatic { "automatic" } else { "manual" }.into()),
        interface_mode: Some(
            completion
                .attempted_modes
                .last()
                .unwrap_or(&profile.interface_mode)
                .clone(),
        ),
        protocol_fallback_enabled: Some(profile.protocol_fallback_enabled),
        attempted_modes: completion.attempted_modes,
        reasoning_effort: profile.reasoning_effort.clone(),
        effective_timeout_seconds: Some(timeout),
        condition_fingerprint: Some(fingerprint),
        error_kind: completion.error_kind,
        http_status: completion.http_status,
        request_id: completion.request_id,
        answer_type: Some(answer_type.as_str().into()),
        answer_parsing_version: Some(html::PARSING_VERSION.into()),
    };
    let result = {
        let mut running = lock(&state.running)?;
        let mut store = lock(&state.store)?;
        let result = store.save_record(&record);
        let mut next = lock(&state.next_auto_test)?;
        // 成功、失败、超时、取消和历史保存失败都在结算后等待完整间隔，不计算请求耗时。
        // 运行门禁尚未释放，外部自动开关在途变化也以这里的最新配置为准，不会抢跑。
        restart_auto_deadline(&store.config.preferences, &mut next, Instant::now());
        drop(next);
        drop(store);
        // 释放运行门禁之前推进通知状态，避免另一个极快请求先结算导致邮件状态倒序。
        // dispatch 只做本地快照/持久化和启动异步任务，不等待 SMTP。
        if result.is_ok() {
            dispatch_notification(&app, &state, record);
        }
        *running = None;
        result
    };
    changed(&app);
    result
}

/// 邮件与请求/持久化生命周期隔离；先消费状态变化，再异步发送且不自动重试。
/// 发送失败不改测试结论，不持有数据库或运行门禁，也不会阻止下次手动测试。
fn dispatch_notification(app: &tauri::AppHandle, state: &AppState, record: TestRecord) {
    // 配置锁内配对 SMTP 目标和对应凭据，禁止并发保存时把新密码发给旧主机。
    let delivery = (|| {
        let store = lock(&state.store)?;
        let mut tracker = lock(&state.notifications)?;
        let mut next = tracker.clone();
        let event = next.consider(&record);
        let payload = serde_json::to_string(&next).map_err(|_| "邮件通知状态保存失败。")?;
        store.save_notification_state(&payload)?;
        *tracker = next;
        let Some(event) = event else {
            return Ok(None);
        };
        let settings = store.config.email_notifications.clone();
        if !event.enabled(&settings) || !settings.has_password {
            return Ok(None);
        }
        let password =
            notifications::read_password()?.ok_or("邮箱授权码不存在，请重新保存邮件设置。")?;
        let generation = state.notification_generation.load(Ordering::Acquire);
        Ok::<_, String>(Some((settings, password, event, generation)))
    })();
    let (settings, password, event, generation) = match delivery {
        Ok(Some(delivery)) => delivery,
        Ok(None) => return,
        Err(message) => {
            if let Ok(mut error) = state.notification_error.lock() {
                *error = Some(message);
            }
            changed(app);
            return;
        }
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = notifications::send_notification(&settings, &password, &record, event).await;
        let state = app.state::<AppState>();
        if let Ok(mut error) = state.notification_error.lock() {
            if state.notification_generation.load(Ordering::Acquire) == generation {
                *error = result.err();
            }
        }
        changed(&app);
    });
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

/// 两个管理按钮共用清空命令；前端确认分类后立即生效，不经“保存设置”再次提交。
/// 数据库一次事务提交；系统凭据/自启/运行时不能加入 SQLite 事务，因此先保存旧值，
/// 任一步失败都尝试补偿恢复并报告恢复失败。网络等待不在此锁范围；已发出的邮件不能撤回。
#[tauri::command]
fn reset_selected_data(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    selection: DataResetSelection,
) -> Result<SaveOutcome, String> {
    let idle = require_idle(&state)?;
    // 与模型拉取/连接诊断共用门禁，避免接口删除后迟到的网络结果重建已清空目录。
    let _diagnostic = if selection.api {
        Some(discovery::DiagnosticGuard::acquire()?)
    } else {
        None
    };
    let mut store = lock(&state.store)?;
    let config = selection.reset_config(&store.config)?;
    conditions::validate_stored_config(&config)?;
    let previous_preferences = store.config.preferences.clone();
    let next_preferences = config.preferences.clone();
    // 必须在任何修改之前读齐回滚材料；读取失败时不执行清空，不输出凭据正文。
    let keys: Vec<(Entry, Option<String>)> = if selection.api {
        store
            .config
            .profiles
            .iter()
            .map(|profile| {
                let entry = credential(&profile.id)?;
                let key = read_key(&entry)?;
                Ok((entry, key))
            })
            .collect::<Result<_, String>>()?
    } else {
        Vec::new()
    };
    let password = if selection.mail {
        notifications::read_password()?
    } else {
        None
    };
    let autostart = if selection.widget {
        Some(autostart::get_autostart()?)
    } else {
        None
    };
    // 提交后的内存状态更新也必须无失败路径；按既有 store → notifications 顺序预先取锁。
    let mut tracker = lock(&state.notifications)?;
    let mut next_auto_test = lock(&state.next_auto_test)?;
    let mut notification_error = lock(&state.notification_error)?;
    let commit = (|| {
        if selection.widget {
            apply_preferences(&app, &next_preferences)?;
            autostart::set_autostart(false)?;
        }
        for (entry, _) in &keys {
            set_key(entry, None)?;
        }
        if selection.mail {
            notifications::save_password(None)?;
        }
        store.reset_selected_data(config, &selection)
    })();
    if let Err(message) = commit {
        let mut failures = Vec::new();
        for (entry, key) in &keys {
            if set_key(entry, key.as_deref()).is_err() {
                failures.push("API Key");
            }
        }
        if selection.mail && notifications::save_password(password.as_deref()).is_err() {
            failures.push("邮箱授权码");
        }
        if let Some(enabled) = autostart {
            if autostart::set_autostart(enabled).is_err() {
                failures.push("系统自启");
            }
            if apply_preferences(&app, &previous_preferences).is_err() {
                failures.push("桌面偏好");
            }
        }
        if failures.is_empty() {
            return Err(message);
        }
        failures.sort_unstable();
        failures.dedup();
        return Err(format!(
            "{message} 部分系统状态恢复失败（{}），请重新配置或重启应用。",
            failures.join("、")
        ));
    }
    if selection.resets_notification_state() {
        *tracker = notifications::NotificationTracker::default();
        *notification_error = None;
        state.notification_generation.fetch_add(1, Ordering::AcqRel);
    }
    if selection.widget || selection.api {
        restart_auto_deadline(&next_preferences, &mut next_auto_test, Instant::now());
    }
    drop(notification_error);
    drop(next_auto_test);
    drop(tracker);
    drop(store);
    // 呈现收尾在提交后执行；失败只返回警告，不能误报清空失败而诱导重复删除。
    let warning = if selection.widget {
        let result = (|| {
            // 放大由前端 dialog 持有生命周期，设置窗口不能只缩小原生宿主而留下放大层。
            // 此时已提交默认位置，保留现有预览，重启再按默认位置显示并明确提示。
            if lock(&state.layout)?.preview_restore.is_some() {
                return Err("当前 HTML 正在放大，默认窗口位置将在重启后生效。".into());
            }
            restore_dock_after_commit(&app, true).map_or(Ok(()), Err)?;
            if let Some(window) = app.get_webview_window("widget") {
                let (_, top, right, _) = work_area(&window)?;
                let scale = window.scale_factor().map_err(|_| "无法读取显示缩放。")?;
                window
                    .set_position(PhysicalPosition::new(
                        right - (382.0 * scale) as i32,
                        top + (90.0 * scale) as i32,
                    ))
                    .map_err(|_| "无法恢复悬浮窗默认位置。")?;
            }
            Ok::<_, String>(())
        })();
        result
            .err()
            .map(|message| format!("所选数据已清空，但窗口恢复未完成：{message} 请重启应用。"))
    } else {
        None
    };
    drop(idle);
    changed(&app);
    Ok(SaveOutcome { warning })
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
    let window = WebviewWindowBuilder::new(
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
    preview::secure(&window)?;
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
    // 桌宠按实际角色轮廓吸附；经典 360px 面板尺寸不参与桌宠判定。
    if lock(&app.state::<pet_window::PetWindowState>().0)?.active {
        return pet_window::snap_scene(&app);
    }
    let window = app.get_webview_window("widget").ok_or("悬浮窗不存在。")?;
    let state = app.state::<AppState>();
    let preserve_layout = {
        let layout = lock(&state.layout)?;
        layout.tucked || layout.preview_restore.is_some()
    };
    if preserve_layout {
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
    if lock(&app.state::<pet_window::PetWindowState>().0)?.active {
        return Ok(());
    }
    if lock(&state.layout)?.preview_restore.is_some() {
        return Ok(());
    }
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
                572
            } else {
                // 卡片新增规则快捷选择行，菜单按较低的规则触发器预留空间。
                156_u32.max(118 + menu_height.min(210))
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

/// 页内预览仅临时放大既有悬浮窗；不创建窗口，原位置/尺寸只留内存。
/// 设置窗口本身足够大，无需调整；重复打开/关闭幂等，失败仍保留恢复依据以便重试。
#[tauri::command]
fn set_html_preview_expanded(
    window: WebviewWindow,
    app: tauri::AppHandle,
    expanded: bool,
) -> Result<(), String> {
    if window.label() == "settings" {
        return Ok(());
    }
    if window.label() != "widget" {
        return Err("此窗口不能放大预览。".into());
    }
    // 放大前先移除裁剪；恢复则必须等尺寸和屏幕位置恢复后再按最终 DPI 建 Region。
    // Region恢复必须在释放layout锁后执行，允许DPI回调读取预览已结束的真实状态。
    if expanded {
        pet_window::preview_region(&app, true)?;
    }
    let state = app.state::<AppState>();
    let mut layout = lock(&state.layout)?;
    if expanded {
        if layout.preview_restore.is_some() {
            return Ok(());
        }
        let position = window.outer_position().map_err(|_| "无法读取窗口位置。")?;
        let size = window.inner_size().map_err(|_| "无法读取窗口尺寸。")?;
        let (left, top, right, bottom) = work_area(&window)?;
        let scale = window.scale_factor().unwrap_or(1.0);
        let width = ((1120.0 * scale) as u32).min((right - left).max(1) as u32);
        let height = ((860.0 * scale) as u32).min((bottom - top).max(1) as u32);
        layout.preview_restore = Some(((position.x, position.y), (size.width, size.height)));
        window
            .set_size(tauri::PhysicalSize::new(width, height))
            .map_err(|_| "无法放大预览。")?;
        window
            .set_position(PhysicalPosition::new(
                left + (right - left - width as i32) / 2,
                top + (bottom - top - height as i32) / 2,
            ))
            .map_err(|_| "无法定位预览。")?;
    } else if let Some((position, size)) = layout.preview_restore {
        window
            .set_size(tauri::PhysicalSize::new(size.0, size.1))
            .map_err(|_| "无法恢复窗口尺寸。")?;
        window
            .set_position(PhysicalPosition::new(position.0, position.1))
            .map_err(|_| "无法恢复窗口位置。")?;
        layout.preview_restore = None;
    }
    drop(layout);
    if !expanded {
        pet_window::preview_region(&app, false)?;
    }
    Ok(())
}

/// 只有用户从拖动区域开始的移动才触发自动收回；程序调整位置不会误触发。
#[tauri::command]
fn prepare_widget_drag(
    app: tauri::AppHandle,
    pointer_delta: Option<[i32; 2]>,
) -> Result<(), String> {
    // 经典标题栏缺省为零；桌宠传入阈值检测已消费的逻辑位移，限制异常 IPC 的位移范围。
    let delta = pointer_delta.unwrap_or([0, 0]);
    if delta
        .iter()
        .any(|value| !(-16_384..=16_384).contains(value))
    {
        return Err("拖动位移超出支持范围。".into());
    }
    let state = app.state::<AppState>();
    let window = app.get_webview_window("widget").ok_or("悬浮窗不存在。")?;
    let position = window.outer_position().map_err(|_| "无法读取拖动起点。")?;
    let scale = window
        .scale_factor()
        .map_err(|_| "无法读取拖动显示比例。")?;
    let generation = {
        let _gate = lock(&DRAG_STATE_GATE)?;
        *lock(&state.drag_origin)? = Some((position.x, position.y));
        // 新拖动和旧保存事务按门禁线性化，旧释放不能借用本次世代建立保存意图。
        state.drag_moved.store(false, Ordering::Relaxed);
        state.dragging.store(true, Ordering::Relaxed);
        state.drag_generation.fetch_add(1, Ordering::Relaxed) + 1
    };
    if delta != [0, 0] {
        let target = PhysicalPosition::new(
            position
                .x
                .saturating_add((f64::from(delta[0]) * scale).round() as i32),
            position
                .y
                .saturating_add((f64::from(delta[1]) * scale).round() as i32),
        );
        if window.set_position(target).is_err() {
            let _ = finish_widget_drag(&app);
            return Err("无法移动桌宠，请重试。".into());
        }
        mark_widget_drag_moved(&app, (target.x, target.y));
    }
    // 即使系统没有产生任何 Moved，也必须在松开鼠标后清理拖动状态。
    // 这条释放监视路径与 Moved 后的收尾共用幂等 finish，单击标题栏不会丢失吸附。
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(40)).await;
            let state = app.state::<AppState>();
            if state.drag_generation.load(Ordering::Relaxed) != generation
                || !state.dragging.load(Ordering::Relaxed)
            {
                return;
            }
            #[cfg(windows)]
            let pressed =
                unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(1) } < 0;
            #[cfg(not(windows))]
            let pressed = false;
            if !pressed {
                if state.drag_moved.load(Ordering::Relaxed) {
                    let _ = snap_widget(app.clone());
                }
                let _ = finish_widget_drag(&app);
                return;
            }
        }
    });
    Ok(())
}

/// 首次收到真实位移后才解除吸附呈现；单击标题栏不会改变边缘状态。
fn mark_widget_drag_moved(app: &tauri::AppHandle, position: (i32, i32)) {
    let state = app.state::<AppState>();
    if !state.dragging.load(Ordering::Relaxed)
        || lock(&state.drag_origin).ok().and_then(|origin| *origin) == Some(position)
        || state.drag_moved.swap(true, Ordering::Relaxed)
    {
        return;
    }
    if let Ok(mut layout) = state.layout.lock() {
        layout.side = None;
    }
    let _ = app.emit(
        "widget-dock",
        DockNotice {
            side: None,
            tucked: false,
        },
    );
}

/// 松开鼠标后识别左、右、顶部，关闭吸附时不产生边缘图标。
fn finish_widget_drag(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let Some((moved, generation)) = release_widget_drag(&state)? else {
        return Ok(());
    };
    let _ = app.emit("widget-drag-ended", ());
    if lock(&app.state::<pet_window::PetWindowState>().0)?.active {
        if moved {
            pet_window::snap_scene(app)?;
            pet_window::queue_position_save(app, generation)?;
        }
        return app
            .emit("widget-dock", pet_window::dock_notice(app)?)
            .map_err(|_| "无法刷新桌宠边缘状态。".into());
    }
    if !moved {
        let layout = lock(&state.layout)?;
        return app
            .emit(
                "widget-dock",
                DockNotice {
                    side: layout.side.clone(),
                    tucked: layout.tucked,
                },
            )
            .map_err(|_| "无法刷新边缘状态。".into());
    }
    let notice = get_widget_dock(app.clone())?;
    app.emit("widget-dock", notice)
        .map_err(|_| "无法刷新边缘状态。".into())
}

/// 释放与位移独立：无位移也结束拖动，重复的系统释放/延迟 Moved 收尾必须幂等。
/// 返回 None 表示无需收尾；元组为是否真实移动、释放所属世代，不能在后续重新读取替代。
fn release_widget_drag(state: &AppState) -> Result<Option<(bool, u64)>, String> {
    let _gate = lock(&DRAG_STATE_GATE)?;
    if !state.dragging.swap(false, Ordering::Relaxed) {
        return Ok(None);
    }
    *lock(&state.drag_origin)? = None;
    Ok(Some((
        state.drag_moved.swap(false, Ordering::Relaxed),
        state.drag_generation.load(Ordering::Relaxed),
    )))
}

/// 退场取消不同于正常松手：作废旧循环并清输入占用，不吸附、不保存位置。
/// 调用方仅在活动桌宠生命周期中使用；经典标题栏仍由 release_widget_drag 正常结算。
#[cfg(test)]
fn cancel_pet_drag_state(state: &AppState) -> Result<bool, String> {
    let gate = lock(&DRAG_STATE_GATE)?;
    cancel_pet_drag_state_locked(state, &gate)
}

/// 调用方已持有生命周期门禁，可与pending清理合并为一个不可被旧保存穿插的取消操作。
fn cancel_pet_drag_state_locked(
    state: &AppState,
    _gate: &MutexGuard<'_, ()>,
) -> Result<bool, String> {
    // 先递增世代，已排队的旧主线程闭包和释放监视器都不得接管之后的新拖动。
    state.drag_generation.fetch_add(1, Ordering::Relaxed);
    // Moved还会排队220ms后的吸附回调；取消必须连这条旧收尾也作废，避免换肤后写位置。
    state.move_generation.fetch_add(1, Ordering::Relaxed);
    let was_dragging = state.dragging.swap(false, Ordering::Relaxed);
    state.drag_moved.store(false, Ordering::Relaxed);
    *lock(&state.drag_origin)? = None;
    Ok(was_dragging)
}

/// 首次订阅时主动读取真实边缘状态，修复启动后尚未拖动的顶部面板不会自动收起。
/// 已缩成图标或正在拖动时保留布局状态，不把小图标坐标覆盖为面板锚点。
#[tauri::command]
fn get_widget_dock(app: tauri::AppHandle) -> Result<DockNotice, String> {
    if lock(&app.state::<pet_window::PetWindowState>().0)?.active {
        return pet_window::dock_notice(&app);
    }
    let state = app.state::<AppState>();
    {
        let layout = lock(&state.layout)?;
        if layout.tucked
            || layout.preview_restore.is_some()
            || state.dragging.load(Ordering::Relaxed)
        {
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
        let mut previous_tick = Instant::now();
        let mut previous_wall = Utc::now();
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let now = Instant::now();
            let wall = Utc::now();
            // 休眠或长请求跨过期限后只恢复一个完整周期，不补齐错过的任务。
            // 同时看单调时间和系统时间，兼容不同平台休眠期间时钟是否前进的差异。
            if scheduler_gap(previous_tick, now, (wall - previous_wall).num_seconds()) {
                if let Ok(store) = app.state::<AppState>().store.lock() {
                    if let Ok(mut next) = app.state::<AppState>().next_auto_test.lock() {
                        *next = now
                            + std::time::Duration::from_secs(
                                store.config.preferences.auto_test_interval_minutes * 60,
                            );
                    }
                }
                changed(&app);
            }
            previous_tick = now;
            previous_wall = wall;
            let due = scheduled_ready(&app.state::<AppState>(), now);
            if due == Ok(true) {
                // 与手动入口共用鉴权、规则快照、总超时和协议回退策略；调度器不额外重试。
                let _ = run_test(app.clone(), app.state::<AppState>(), true).await;
                // run_test 已统一从结算时刻重新计时；网络耗时不能被当作休眠再次推迟期限。
                previous_tick = Instant::now();
                previous_wall = Utc::now();
            }
        }
    });
}

/// 正常每秒轮询；超过十秒表示执行已暂停，系统时钟回拨本身不触发补跑。
fn scheduler_gap(previous: Instant, now: Instant, wall_seconds: i64) -> bool {
    now.saturating_duration_since(previous) > std::time::Duration::from_secs(10)
        || wall_seconds > 10
}

/// 请求结算后无条件重置一个完整周期；在途重新开启自动测试也从结算后计时，不补跑。
fn restart_auto_deadline(preferences: &Preferences, next: &mut Instant, now: Instant) {
    *next = now + std::time::Duration::from_secs(preferences.auto_test_interval_minutes * 60);
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

/// 测试开始时预占期限，防止同一到期轮次被重复领取；这不是界面倒计时起点。
/// 手动请求始终可领取，自动请求须启用且到期；结算时无条件重置完整周期，测试中不展示期限。
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

/// 托盘和重复启动共用显示逻辑，恢复时夹紧工作区，解决显示器拔出后的不可见窗口。
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
        .manage(pet_window::PetWindowState::default())
        .manage(preview::PreviewState::default())
        .register_uri_scheme_protocol("html-preview", |context, request| {
            preview::respond(
                context.app_handle(),
                context.webview_label(),
                &request.uri().to_string(),
            )
        })
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_widget(app)
        }))
        .setup(|app| {
            let folder = app.path().app_data_dir()?;
            std::fs::create_dir_all(&folder)?;
            let store = Store::open(&folder.join("probe.db")).map_err(std::io::Error::other)?;
            let position = store.config.widget_position;
            let preferences = store.config.preferences.clone();
            let notification_tracker = store
                .load_notification_tracker()
                .map_err(std::io::Error::other)?;
            app.manage(AppState {
                store: Mutex::new(store),
                running: Mutex::new(None),
                move_generation: AtomicU64::new(0),
                settings_gate: Mutex::new(()),
                layout: Mutex::new(WidgetLayout::default()),
                dragging: AtomicBool::new(false),
                drag_moved: AtomicBool::new(false),
                drag_generation: AtomicU64::new(0),
                drag_origin: Mutex::new(None),
                notifications: Mutex::new(notification_tracker),
                notification_error: Mutex::new(None),
                notification_generation: AtomicU64::new(0),
                next_auto_test: Mutex::new(
                    Instant::now()
                        + std::time::Duration::from_secs(
                            preferences.auto_test_interval_minutes * 60,
                        ),
                ),
            });
            let handle = app.handle();
            apply_preferences(handle, &preferences).map_err(std::io::Error::other)?;
            let window = app
                .get_webview_window("widget")
                .ok_or("悬浮窗初始化失败。")?;
            preview::secure(&window).map_err(std::io::Error::other)?;
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
            // 桌宠资源尚在前端加载时不能先按经典 360px 面板吸附/夹紧，
            // 否则屏幕右侧保存的 182px 桌宠位置会在每次重启时被向左推。
            // 首个已验证场景会按真实尺寸夹紧；坏皮肤回退时仍由经典布局负责恢复。
            if preferences.skin.id == "classic" {
                snap_widget(handle.clone()).map_err(std::io::Error::other)?;
            }
            use tauri::{
                menu::{CheckMenuItem, Menu, MenuItem},
                tray::TrayIconBuilder,
            };
            let show = MenuItem::with_id(app, "show", "显示悬浮组件", true, None::<&str>)?;
            let auto_test = CheckMenuItem::with_id(
                app,
                "auto-test",
                "自动测试",
                true,
                preferences.auto_test_enabled,
                None::<&str>,
            )?;
            // 设置页和悬浮窗保存后广播快照；托盘复选项据同一持久化值刷新，
            // 避免从不同入口切换后勾选状态滞留。
            let sync_app = app.handle().clone();
            let sync_item = auto_test.clone();
            let listener_app = sync_app.clone();
            sync_app.listen("snapshot-changed", move |_| {
                if let Ok(store) = listener_app.state::<AppState>().store.lock() {
                    let _ = sync_item.set_checked(store.config.preferences.auto_test_enabled);
                }
            });
            let settings = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出测智雷达", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &auto_test, &settings, &quit])?;
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
                "auto-test" => {
                    let state = app.state::<AppState>();
                    let enabled = state
                        .store
                        .lock()
                        .map(|store| !store.config.preferences.auto_test_enabled)
                        .unwrap_or(false);
                    if set_auto_test_enabled(app.clone(), state, enabled).is_err() {
                        // 原生复选项可能已自行翻转；写盘失败时从权威快照恢复勾选。
                        changed(app);
                    }
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
            updates::start_monitor(handle.clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                // 关闭设置窗口时由原生生命周期兜底清理，不依赖定时过期或 Vue 卸载执行。
                window
                    .app_handle()
                    .state::<preview::PreviewState>()
                    .release_owner(window.label());
                return;
            }
            if window.label() != "widget" {
                return;
            }
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = pet_window::cancel_active_drag(window.app_handle());
                    let _ = window.hide();
                }
                tauri::WindowEvent::ScaleFactorChanged { .. } => {
                    pet_window::refresh_region(window.app_handle());
                }
                tauri::WindowEvent::Moved(position) => {
                    // 拖动结束 220ms 后再吸附和持久化，避免每个鼠标移动都执行写盘。
                    let app = window.app_handle().clone();
                    mark_widget_drag_moved(&app, (position.x, position.y));
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
            pet_window::set_pet_scene_layout,
            pet_window::get_pet_menu_bounds,
            pet_window::clear_pet_scene,
            pet_window::start_pet_drag,
            pet_window::cancel_pet_drag,
            pet_window::list_skins,
            pet_window::get_skin,
            pet_window::install_skin,
            pet_window::remove_skin,
            preview::prepare_html_preview,
            preview::release_html_preview,
            set_html_preview_expanded,
            autostart::get_autostart,
            autostart::set_autostart,
            updates::open_external_url,
            updates::get_update_status,
            updates::check_updates,
            fetch_models,
            get_model_catalog,
            fetch_profile_models,
            set_profile_model,
            test_connection,
            get_snapshot,
            get_history_index,
            get_history_records,
            search_history,
            get_widget_dock,
            save_profile,
            delete_profile,
            select_profile,
            save_rule,
            save_rule_preset,
            set_profile_rule,
            set_profile_effort,
            delete_rule_preset,
            save_preferences,
            preview_history_retention,
            export_config_backup,
            preview_config_import,
            import_config_backup,
            save_email_settings,
            send_test_email,
            set_auto_test_enabled,
            start_test,
            cancel_test,
            clear_history,
            reset_selected_data,
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

    /// 只对实际鉴权条件变化换修订；不泄露密钥，重复保存与残留凭据不误失效。
    #[test]
    fn credential_changes_invalidate_results_without_storing_the_key() {
        let mut profile: ModelProfile = serde_json::from_value(serde_json::json!({
            "id":"one", "name":"fixture", "baseUrl":"https://example.com/v1", "model":"fixture", "hasKey":true,
            "credentialRevision":"previous-revision"
        })).unwrap();
        assert_eq!(
            next_credential_revision(Some(&profile), Some("fixture-old"), Some("fixture-old"))
                .as_deref(),
            Some("previous-revision")
        );
        let replaced =
            next_credential_revision(Some(&profile), Some("fixture-old"), Some("fixture-new"))
                .unwrap();
        assert_ne!(replaced, "previous-revision");
        assert!(!replaced.contains("fixture"));
        assert!(next_credential_revision(Some(&profile), Some("fixture-old"), None).is_some());
        assert!(next_credential_revision(Some(&profile), None, None).is_some());
        profile.has_key = false;
        assert_eq!(
            next_credential_revision(Some(&profile), Some("orphaned-fixture"), None).as_deref(),
            Some("previous-revision")
        );
        assert!(next_credential_revision(None, None, None).is_none());
        assert!(next_credential_revision(None, None, Some("fixture-new")).is_some());
    }

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

    /// 退场清理作废旧循环且只结算一次；不复用正常释放的吸附、写盘或经典边缘恢复。
    #[test]
    fn pet_drag_cancel_invalidates_old_generation_without_saving_or_clearing_classic_side() {
        let state = AppState {
            store: Mutex::new(Store::open(std::path::Path::new(":memory:")).unwrap()),
            running: Mutex::new(None),
            move_generation: AtomicU64::new(0),
            settings_gate: Mutex::new(()),
            layout: Mutex::new(WidgetLayout::default()),
            dragging: AtomicBool::new(true),
            drag_moved: AtomicBool::new(true),
            drag_generation: AtomicU64::new(4),
            drag_origin: Mutex::new(Some((10, 20))),
            notifications: Mutex::new(notifications::NotificationTracker::default()),
            notification_error: Mutex::new(None),
            notification_generation: AtomicU64::new(0),
            next_auto_test: Mutex::new(Instant::now()),
        };
        state.layout.lock().unwrap().side = Some("left".into());
        let position = state.store.lock().unwrap().config.widget_position;
        assert!(cancel_pet_drag_state(&state).unwrap());
        assert_eq!(state.drag_generation.load(Ordering::Relaxed), 5);
        assert_eq!(state.move_generation.load(Ordering::Relaxed), 1);
        assert!(!state.dragging.load(Ordering::Relaxed));
        assert!(!state.drag_moved.load(Ordering::Relaxed));
        assert!(state.drag_origin.lock().unwrap().is_none());
        assert_eq!(state.store.lock().unwrap().config.widget_position, position);
        assert_eq!(state.layout.lock().unwrap().side.as_deref(), Some("left"));
        assert!(!cancel_pet_drag_state(&state).unwrap());
        assert_eq!(release_widget_drag(&state).unwrap(), None);
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
            drag_moved: AtomicBool::new(false),
            drag_generation: AtomicU64::new(0),
            drag_origin: Mutex::new(None),
            notifications: Mutex::new(notifications::NotificationTracker::default()),
            notification_error: Mutex::new(None),
            notification_generation: AtomicU64::new(0),
            next_auto_test: Mutex::new(now),
        };
        assert!(!scheduled_ready(&state, now).unwrap());
        // 复用真实状态，覆盖“边缘标题栏只按下又松开、没有任何 Moved”的路径。
        state.layout.lock().unwrap().side = Some("top".into());
        state.dragging.store(true, Ordering::Relaxed);
        *state.drag_origin.lock().unwrap() = Some((700, 0));
        assert_eq!(release_widget_drag(&state).unwrap(), Some((false, 0)));
        assert!(!state.dragging.load(Ordering::Relaxed));
        assert_eq!(state.layout.lock().unwrap().side.as_deref(), Some("top"));
        assert!(state.drag_origin.lock().unwrap().is_none());
        assert_eq!(release_widget_drag(&state).unwrap(), None);
        {
            let mut store = state.store.lock().unwrap();
            store.config.profiles.push(ModelProfile {
                id: "one".into(),
                name: "测试模型".into(),
                base_url: "http://localhost/v1".into(),
                model: "fixture".into(),
                has_key: false,
                credential_revision: None,
                interface_mode: "chat_completions".into(),
                protocol_fallback_enabled: false,
                reasoning_effort: Some("low".into()),
                timeout_seconds: None,
                rule_preset_id: None,
                catalog_token: None,
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

    /// 外观类设置保存不应推迟自动测试；开关或间隔变化才重新计算周期。
    #[test]
    fn unrelated_preferences_keep_auto_deadline() {
        let previous = Preferences::default();
        let mut visual = previous.clone();
        visual.opacity = 90;
        visual.always_on_top = false;
        assert!(!auto_schedule_changed(&previous, &visual));

        let mut disabled = previous.clone();
        disabled.auto_test_enabled = false;
        assert!(auto_schedule_changed(&previous, &disabled));

        let mut interval = previous.clone();
        interval.auto_test_interval_minutes = 5;
        assert!(auto_schedule_changed(&previous, &interval));
    }

    /// 唤醒只重置一个未来周期，不把错过的间隔当队列；时钟回拨不会密集补跑。
    #[test]
    fn scheduler_detects_suspend_without_treating_clock_rollback_as_backlog() {
        let now = Instant::now();
        assert!(!scheduler_gap(
            now,
            now + std::time::Duration::from_secs(1),
            1
        ));
        assert!(!scheduler_gap(
            now,
            now + std::time::Duration::from_secs(1),
            -3600
        ));
        assert!(scheduler_gap(
            now,
            now + std::time::Duration::from_secs(3600),
            3600
        ));
        assert!(scheduler_gap(
            now,
            now + std::time::Duration::from_secs(1),
            3600
        ));
        let mut next = now;
        let resumed = now + std::time::Duration::from_secs(3600);
        assert!(claim_test_deadline(
            &Preferences::default(),
            &mut next,
            resumed,
            true
        ));
        assert_eq!(next, resumed + std::time::Duration::from_secs(180));
        assert!(!claim_test_deadline(
            &Preferences::default(),
            &mut next,
            resumed,
            true
        ));
    }

    /// 所有结算统一重计完整间隔；请求中重新开启产生的未来期限也不能算入本次等待。
    #[test]
    fn settled_run_restarts_full_interval_even_after_inflight_toggle() {
        let now = Instant::now();
        let preferences = Preferences::default();
        let mut expired = now - std::time::Duration::from_secs(20);
        restart_auto_deadline(&preferences, &mut expired, now);
        assert_eq!(expired, now + std::time::Duration::from_secs(180));
        let future = now + std::time::Duration::from_secs(120);
        let mut next = future;
        restart_auto_deadline(&preferences, &mut next, now);
        assert_eq!(next, now + std::time::Duration::from_secs(180));
        assert_eq!(Preferences::default().timeout_seconds, 300);
    }

    /// 老配置补齐新增字段，同时保留原有关闭选项。
    #[test]
    fn old_preferences_keep_values_and_get_schedule_defaults() {
        let preferences: Preferences = serde_json::from_value(serde_json::json!({"alwaysOnTop":false,"snapToEdge":false,"opacity":95,"timeoutSeconds":30,"shortcutEnabled":false})).unwrap();
        assert_eq!(preferences.timeout_seconds, 30);
        assert!(!preferences.always_on_top);
        assert!(preferences.auto_test_enabled);
        assert_eq!(preferences.auto_test_interval_minutes, 3);
    }
}
