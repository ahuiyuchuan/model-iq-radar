//! 关于页的只读正式发布检查与系统浏览器边界。仅用户点击时联网，不接收模型配置。
//! 不下载/执行程序，不发送题目、历史或凭据，不持有测试和存储锁。作者：ahui。

use reqwest::{redirect::Policy, Client, Response, StatusCode};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use url::Url;

const REPOSITORY: &str = "https://github.com/ahuiyuchuan/model-iq-radar";
const API_REPOSITORY: &str = "https://api.github.com/repos/ahuiyuchuan/model-iq-radar";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 只读更新结果；status 为 available / current / no-release，失败以 IPC 错误返回。
#[derive(Serialize)]
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

/// 固定公开仓库；只有明确点击检查才请求，不读取模型地址、配置或 API Key。
#[tauri::command]
pub async fn check_updates() -> Result<UpdateResult, String> {
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
