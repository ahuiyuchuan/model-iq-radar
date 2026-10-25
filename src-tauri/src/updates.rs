//! 进程级每小时静默检查与用户主动检查共用公开发布请求，与模型配置隔离。
//! 不下载/执行程序，不发送题目、历史或凭据，不持有测试和存储锁。作者：ahui。

use reqwest::{redirect::Policy, Client, Response, StatusCode};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{Emitter, Manager, State};
use url::Url;

const REPOSITORY: &str = "https://github.com/ahuiyuchuan/model-iq-radar";
const API_REPOSITORY: &str = "https://api.github.com/repos/ahuiyuchuan/model-iq-radar";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
/// 检查结束后等待完整一小时；失败、唤醒、窗口重开都不能引起密集重试或补跑。
const CHECK_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// 网络请求合并门禁与公开发布缓存；不接触模型运行锁或业务数据库。
#[derive(Default)]
pub struct UpdateCache {
    state: Mutex<CacheState>,
    request: tokio::sync::Mutex<()>,
}

#[derive(Default)]
struct CacheState {
    latest: Option<UpdateResult>,
    generation: u64,
    /// 仅供已等待同一次请求的调用方共享失败；新的一次主动检查仍可重试。
    outcome: Option<Result<UpdateResult, String>>,
}

impl UpdateCache {
    /// 同时到来的后台/手动调用共用一次网络请求；失败保留最近成功结果。
    /// 返回值中的 bool 仅在成功缓存内容变化时为 true，防止重复广播。
    async fn check<F, R>(&self, check: F) -> Result<(UpdateResult, bool), String>
    where
        F: FnOnce() -> R,
        R: Future<Output = Result<UpdateResult, String>>,
    {
        let generation = self
            .state
            .lock()
            .map_err(|_| "更新缓存暂不可用。")?
            .generation;
        let _request = self.request.lock().await;
        {
            let state = self.state.lock().map_err(|_| "更新缓存暂不可用。")?;
            if state.generation != generation {
                if let Some(outcome) = &state.outcome {
                    return outcome.clone().map(|result| (result, false));
                }
            }
        }
        let outcome = check().await;
        let mut state = self.state.lock().map_err(|_| "更新缓存暂不可用。")?;
        let changed = outcome
            .as_ref()
            .is_ok_and(|result| state.latest.as_ref() != Some(result));
        if let Ok(result) = &outcome {
            state.latest = Some(result.clone());
        }
        state.generation = state.generation.wrapping_add(1);
        state.outcome = Some(outcome.clone());
        outcome.map(|result| (result, changed))
    }
}

/// 设置页只读内存缓存，不触发网络检查。缓存异常按静默约定由前端忽略。
#[tauri::command]
pub fn get_update_status(state: State<'_, UpdateCache>) -> Result<Option<UpdateResult>, String> {
    state
        .state
        .lock()
        .map(|value| value.latest.clone())
        .map_err(|_| "更新缓存暂不可用。".into())
}

/// setup 仅启动一个后台任务；打开多个窗口或重开设置不创建新的检查器。
/// 无论成功失败都不弹通知、不自动下载，也不持有测试运行门禁或存储锁。
pub fn start_monitor(app: tauri::AppHandle) {
    if !app.manage(UpdateCache::default()) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        monitor_loop(|| check_updates(app.clone()), |_| {}).await;
    });
}

/// 串行检查、完成后计时；不用 interval 的追赶行为，休眠错过的轮次直接跳过。
/// 将只读检查与发布回调分离，便于用虚拟时钟验证失败节流且不请求真实 GitHub。
async fn monitor_loop<F, R, P>(mut check: F, mut publish: P)
where
    F: FnMut() -> R,
    R: Future<Output = Result<UpdateResult, String>>,
    P: FnMut(UpdateResult),
{
    loop {
        if let Ok(result) = check().await {
            publish(result);
        }
        tokio::time::sleep(CHECK_INTERVAL).await;
    }
}

/// 只读更新结果；后台失败不反馈，主动检查失败仅返回给主动调用的界面。
#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResult {
    status: &'static str,
    current_version: String,
    latest_version: Option<String>,
    release_url: String,
    notes: String,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    draft: bool,
    prerelease: bool,
    body: Option<String>,
}

/// 对系统浏览器入口再次验证；拒绝控制字符、相对地址、本地/脚本协议和嵌入凭据。
pub fn external_url(value: &str) -> Result<Url, String> {
    if value.len() > 8192
        || value.chars().any(|c| c <= ' ' || c == '\u{7f}')
        || !(value.to_ascii_lowercase().starts_with("https://")
            || value.to_ascii_lowercase().starts_with("http://"))
    {
        return Err("只允许打开完整的 HTTP 或 HTTPS 网页地址。".into());
    }
    let url = Url::parse(value).map_err(|_| "网页地址无效。")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("网页地址包含不允许的协议或凭据信息。".into());
    }
    Ok(url)
}

/// ShellExecuteW 仅接收验证过的 URL，绝不经过 cmd/PowerShell；失败不会影响模型任务。
#[tauri::command]
pub fn open_external_url(url: String) -> Result<(), String> {
    let url = external_url(&url)?;
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        let verb: Vec<u16> = "open\0".encode_utf16().collect();
        let target: Vec<u16> = url.as_str().encode_utf16().chain(Some(0)).collect();
        // verb 和 target 以 NUL 终止，在调用期间有效；不提供可执行参数和工作目录。
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                target.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        };
        if result as isize <= 32 {
            return Err("无法打开系统默认浏览器，请检查系统关联。".into());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = url;
        Err("此免安装版本仅支持 Windows。".into())
    }
}

/// 主动检查与后台共用请求门禁；成功同步所有窗口，错误只由主动调用方显示。
#[tauri::command]
pub async fn check_updates(app: tauri::AppHandle) -> Result<UpdateResult, String> {
    let cache = app.state::<UpdateCache>();
    let (result, changed) = cache.check(fetch_updates).await?;
    if changed {
        let _ = app.emit("update-status-changed", &result);
    }
    Ok(result)
}

/// 固定公开仓库、单次总预算，不读取模型地址、配置或 API Key。
async fn fetch_updates() -> Result<UpdateResult, String> {
    let client = Client::builder()
        .redirect(Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .user_agent(concat!("Model-IQ-Radar/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| "无法初始化更新检查。")?;
    tokio::time::timeout(
        Duration::from_secs(12),
        fetch_latest(
            &client,
            &format!("{API_REPOSITORY}/releases/latest"),
            API_REPOSITORY,
            CURRENT_VERSION,
        ),
    )
    .await
    .map_err(|_| "检查更新超时，请稍后重试。".to_string())?
}

/// 404 时另查仓库是否可访问，避免将私有/删除仓库误报为无发布；请求总预算由外层限制。
async fn fetch_latest(
    client: &Client,
    endpoint: &str,
    repository_endpoint: &str,
    current: &str,
) -> Result<UpdateResult, String> {
    let response = client
        .get(endpoint)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|_| "检查更新失败，请检查网络、代理或 TLS 证书。")?;
    if response.status() == StatusCode::NOT_FOUND {
        let repository = client
            .get(repository_endpoint)
            .send()
            .await
            .map_err(|_| "无法核实更新仓库，请稍后重试。")?;
        ensure_success(repository.status())?;
        return Ok(UpdateResult {
            status: "no-release",
            current_version: current.into(),
            latest_version: None,
            release_url: format!("{REPOSITORY}/releases"),
            notes: String::new(),
        });
    }
    ensure_success(response.status())?;
    parse_release(&read_limited(response).await?, current)
}

/// 状态错误不回显任意服务端文本、URL 或请求头；限流不能显示为“已是最新版”。
fn ensure_success(status: StatusCode) -> Result<(), String> {
    if status.is_success() {
        return Ok(());
    }
    Err(match status.as_u16() {
        403 | 429 => "GitHub 暂时限制更新检查，请稍后重试。".into(),
        404 => "更新仓库不存在或暂时不可访问。".into(),
        300..=399 => "更新服务返回重定向，已停止检查。".into(),
        value => format!("更新服务返回 HTTP {value}，请稍后重试。"),
    })
}

/// 流式限制 512 KiB，避免未知响应占满内存；只读取发布元数据，不下载资产。
async fn read_limited(mut response: Response) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| "更新信息读取中断。")? {
        if bytes.len() + chunk.len() > 512 * 1024 {
            return Err("更新信息过大，已停止检查。".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// 使用语义版本而非字符串比较；拒绝草稿、预发布、无效版本和外部伪造的发布链接。
fn parse_release(bytes: &[u8], current: &str) -> Result<UpdateResult, String> {
    let release: Release = serde_json::from_slice(bytes).map_err(|_| "更新信息格式无效。")?;
    let latest = Version::parse(
        release
            .tag_name
            .strip_prefix('v')
            .unwrap_or(&release.tag_name),
    )
    .map_err(|_| "发布版本号格式无效。")?;
    let current = Version::parse(current).map_err(|_| "当前版本号格式无效。")?;
    let page = external_url(&release.html_url)?;
    if release.draft || release.prerelease || !latest.pre.is_empty() {
        return Err("更新服务未返回正式发布版本。".into());
    }
    if page.scheme() != "https"
        || page.host_str() != Some("github.com")
        || page.port().is_some()
        || page.query().is_some()
        || page.fragment().is_some()
        || !page
            .path()
            .starts_with("/ahuiyuchuan/model-iq-radar/releases/tag/")
    {
        return Err("发布页面地址不属于测智雷达项目。".into());
    }
    Ok(UpdateResult {
        status: if latest.cmp_precedence(&current).is_gt() {
            "available"
        } else {
            "current"
        },
        current_version: current.to_string(),
        latest_version: Some(latest.to_string()),
        release_url: page.into(),
        notes: release
            .body
            .unwrap_or_default()
            .chars()
            .take(4000)
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn available() -> UpdateResult {
        UpdateResult {
            status: "available",
            current_version: "1.0.1".into(),
            latest_version: Some("1.1.0".into()),
            release_url: REPOSITORY.into(),
            notes: "公开发布说明".into(),
        }
    }

    /// 后台和多个窗口同时主动检查只发一次请求，成功结果变化也只广播一次。
    #[tokio::test(start_paused = true)]
    async fn concurrent_checks_share_one_request_and_one_change() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let cache = UpdateCache::default();
        let calls = AtomicUsize::new(0);
        let fetch = || async {
            calls.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_secs(1)).await;
            Ok(available())
        };
        let (first, second, third) =
            tokio::join!(cache.check(fetch), cache.check(fetch), cache.check(fetch));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(first.unwrap().1);
        assert!(!second.unwrap().1);
        assert!(!third.unwrap().1);
        assert!(!cache.check(fetch).await.unwrap().1);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    /// 失败合并给当前等待者，保留成功缓存；之后的主动点击可重新检查，不被失败缓存锁死。
    #[tokio::test(start_paused = true)]
    async fn shared_failure_preserves_success_and_later_click_can_retry() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let cache = UpdateCache::default();
        cache.check(|| async { Ok(available()) }).await.unwrap();
        let calls = AtomicUsize::new(0);
        let fail = || async {
            calls.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_secs(1)).await;
            Err("模拟离线".into())
        };
        let (first, second) = tokio::join!(cache.check(fail), cache.check(fail));
        assert!(first.is_err());
        assert!(second.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            cache.state.lock().unwrap().latest.as_ref().unwrap().status,
            "available"
        );
        assert!(!cache.check(|| async { Ok(available()) }).await.unwrap().1);
        assert!(cache
            .state
            .lock()
            .unwrap()
            .outcome
            .as_ref()
            .unwrap()
            .is_ok());
    }

    /// 首次检查立即执行；失败也等一小时，跨多小时恢复仅执行一次，不补跑历史轮次。
    #[tokio::test(start_paused = true)]
    async fn hourly_monitor_is_silent_on_failure_and_skips_missed_checks() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        let attempts = Arc::new(AtomicUsize::new(0));
        let published = Arc::new(AtomicUsize::new(0));
        let counter = attempts.clone();
        let notices = published.clone();
        let worker = tokio::spawn(monitor_loop(
            move || {
                let index = counter.fetch_add(1, Ordering::SeqCst);
                std::future::ready(if index == 0 {
                    Err("离线".into())
                } else {
                    Ok(UpdateResult {
                        status: "available",
                        current_version: "1.0.1".into(),
                        latest_version: Some("1.1.0".into()),
                        release_url: REPOSITORY.into(),
                        notes: String::new(),
                    })
                })
            },
            move |_| {
                notices.fetch_add(1, Ordering::SeqCst);
            },
        ));
        tokio::task::yield_now().await;
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        assert_eq!(published.load(Ordering::SeqCst), 0);
        tokio::time::advance(CHECK_INTERVAL - Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        tokio::time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert_eq!(published.load(Ordering::SeqCst), 1);
        tokio::time::advance(CHECK_INTERVAL * 12).await;
        tokio::task::yield_now().await;
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
        tokio::task::yield_now().await;
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
        worker.abort();
    }

    /// 恶意协议、控制字符和本机文件在原生边界也被拒绝，不能仅依赖 DOM 过滤。
    #[test]
    fn browser_boundary_rejects_non_web_targets() {
        for value in [
            "javascript:alert(1)",
            "data:text/html,test",
            "file:///C:/test.exe",
            "shell:AppsFolder",
            "//example.com",
            "https://a:b@example.com",
            "https://example.com\nfoo",
        ] {
            assert!(external_url(value).is_err(), "{value}");
        }
        assert_eq!(
            external_url("https://example.com/a?q=x#ok")
                .unwrap()
                .scheme(),
            "https"
        );
    }

    /// 版本排序、同版重打包不误报更新、预发布和伪造下载地址均在真实解析器中验证。
    #[test]
    fn compares_release_versions_and_rejects_invalid_metadata() {
        let mut value = json!({"tag_name":"v1.10.0","html_url":format!("{REPOSITORY}/releases/tag/v1.10.0"),"draft":false,"prerelease":false,"body":"更新说明"});
        assert_eq!(
            parse_release(value.to_string().as_bytes(), "1.9.0")
                .unwrap()
                .status,
            "available"
        );
        assert_eq!(
            parse_release(value.to_string().as_bytes(), "1.10.0")
                .unwrap()
                .status,
            "current"
        );
        assert_eq!(
            parse_release(value.to_string().as_bytes(), "2.0.0")
                .unwrap()
                .status,
            "current"
        );
        value["prerelease"] = json!(true);
        assert!(parse_release(value.to_string().as_bytes(), "1.0.0").is_err());
        value["prerelease"] = json!(false);
        value["html_url"] = json!("https://evil.example/download.exe");
        assert!(parse_release(value.to_string().as_bytes(), "1.0.0").is_err());
        assert!(parse_release(b"not-json", "1.0.0").is_err());
        assert!(ensure_success(StatusCode::TOO_MANY_REQUESTS).is_err());
    }

    /// 真正经过 HTTP 客户端验证无发布和仓库不可访问的区别，且不发送模型数据。
    #[tokio::test]
    async fn checks_empty_releases_without_claiming_current() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        for (repository_status, expected_ok) in [("200 OK", true), ("404 Not Found", false)] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let worker = std::thread::spawn(move || {
                for status in ["404 Not Found", repository_status] {
                    let (mut stream, _) = listener.accept().unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(3)))
                        .unwrap();
                    let mut request = [0; 4096];
                    let count = stream.read(&mut request).unwrap();
                    let request = String::from_utf8_lossy(&request[..count]);
                    assert!(!request.to_lowercase().contains("authorization:"));
                    stream.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}").as_bytes()).unwrap();
                }
            });
            let client = Client::builder()
                .timeout(Duration::from_secs(3))
                .build()
                .unwrap();
            let result = fetch_latest(&client, &format!("{base}/latest"), &base, "1.0.0").await;
            assert_eq!(result.is_ok(), expected_ok);
            if let Ok(value) = result {
                assert_eq!(value.status, "no-release");
            }
            worker.join().unwrap();
        }
    }
}
