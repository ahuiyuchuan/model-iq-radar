//! 接口配置页的临时诊断，不写配置/历史，不重置自动测试，不读取模型回答作为指令。
//! 参考 CC Switch 的 /models 发现与可编辑模型选择，限于本项目 Chat Completions 协议。
//! 密钥由调用端短暂传入，只发往经过现有 API 地址验证的同一服务。作者：ahui。

use crate::{
    api,
    model::{ModelProfile, TestRule},
};
use reqwest::{redirect::Policy, Client, StatusCode};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;
use url::Url;

/// 仅约束用户主动诊断，独立于付费测智任务；跨设置窗口也不重复发送诊断。
static DIAGNOSTIC_RUNNING: AtomicBool = AtomicBool::new(false);
pub struct DiagnosticGuard;
impl DiagnosticGuard {
    pub fn acquire() -> Result<Self, String> {
        DIAGNOSTIC_RUNNING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "接口诊断正在进行，请稍候。")?;
        Ok(Self)
    }
}
impl Drop for DiagnosticGuard {
    fn drop(&mut self) {
        DIAGNOSTIC_RUNNING.store(false, Ordering::Release);
    }
}

/// 返回可选择的模型 ID 与可选服务商信息，不伪造“可调用”或配额承诺。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchedModel {
    pub id: String,
    pub owned_by: Option<String>,
}

/// 真实完成一次轻量 Chat Completions 请求的耗时；不产生测智结论。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionResult {
    duration_ms: u64,
}

/// 从已支持的根路径/完整 completions URL 推导同一服务的模型路径，不猜测其他域名。
fn models_endpoint(base: &str) -> Result<Url, String> {
    let mut url = api::endpoint(base)?;
    let root = url
        .path()
        .strip_suffix("/chat/completions")
        .unwrap_or("")
        .to_string();
    url.set_path(&format!("{root}/models"));
    Ok(url)
}

/// GET /models，仅列表请求，无模型推理；15秒总体超时、1MiB上限、不重定向、不重试。
pub async fn fetch_models(base: &str, key: &str) -> Result<Vec<FetchedModel>, String> {
    let address = models_endpoint(base)?;
    let work = async {
        let client = Client::builder()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .build()
            .map_err(|_| "无法初始化模型列表请求。")?;
        let mut request = client.get(address);
        if !key.is_empty() {
            request = request.bearer_auth(key);
        }
        let mut response = request
            .send()
            .await
            .map_err(|_| "拉取模型失败，请检查地址、网络或证书。")?;
        if !response.status().is_success() {
            return Err(match response.status() {
                StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                    "鉴权失败，请检查 API Key 及模型列表权限。".into()
                }
                StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED => {
                    "接口不提供 /models 列表，请手动填写模型 ID。".into()
                }
                StatusCode::TOO_MANY_REQUESTS => "接口请求被限流，请稍后重试。".into(),
                status if status.is_redirection() => "接口返回重定向，已停止发送凭据。".into(),
                status => format!("模型列表返回 HTTP {}。", status.as_u16()),
            });
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| "模型列表读取中断。")? {
            if bytes.len() + chunk.len() > 1024 * 1024 {
                return Err("模型列表超过 1 MiB，已停止读取。".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        parse_models(&bytes, key)
    };
    tokio::time::timeout(Duration::from_secs(15), work)
        .await
        .map_err(|_| "拉取模型超过15秒，请稍后重试。".to_string())?
}

/// 兼容标准 data[].id 和部分聚合服务 models[].slug/id；排序去重，过滤无效/过长 ID。
fn parse_models(bytes: &[u8], key: &str) -> Result<Vec<FetchedModel>, String> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| "模型列表不是有效 JSON。")?;
    let items = value
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| value.get("models").and_then(Value::as_array))
        .ok_or("模型列表缺少 data 或 models 数组。")?;
    let mut models = BTreeMap::new();
    for item in items.iter().take(5000) {
        let Some(id) = item
            .get("id")
            .and_then(Value::as_str)
            .or_else(|| item.get("slug").and_then(Value::as_str))
        else {
            continue;
        };
        if id.trim().is_empty()
            || id.chars().count() > 200
            || id.chars().any(char::is_control)
            || (!key.is_empty() && id.contains(key))
        {
            continue;
        }
        let owned_by = item
            .get("owned_by")
            .and_then(Value::as_str)
            .filter(|s| {
                s.chars().count() <= 80
                    && !s.chars().any(char::is_control)
                    && (key.is_empty() || !s.contains(key))
            })
            .map(String::from);
        models.entry(id.to_string()).or_insert(FetchedModel {
            id: id.into(),
            owned_by,
        });
    }
    Ok(models.into_values().collect())
}

/// 使用固定独立消息验证连接，timeout_seconds 为已保存的测智超时（秒），不发送测试规则。
pub async fn test_connection(
    base: &str,
    model: &str,
    key: &str,
    timeout_seconds: u64,
) -> Result<ConnectionResult, String> {
    if model.trim().is_empty() || model.chars().count() > 200 {
        return Err("请先填写或选择模型 ID。".into());
    }
    let start = Instant::now();
    let profile = ModelProfile {
        id: String::new(),
        name: String::new(),
        base_url: base.into(),
        model: model.into(),
        has_key: !key.is_empty(),
    };
    let rule = TestRule {
        prompt: "请只回复 OK。".into(),
        keyword: "OK".into(),
    };
    let result = api::complete(
        &profile,
        &rule,
        key,
        timeout_seconds,
        &CancellationToken::new(),
    )
    .await;
    if let Some(error) = result.error {
        return Err(error);
    }
    Ok(ConnectionResult {
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 连接诊断必须使用传入的测智超时，不能继续固定15秒；仅本机延迟响应。
    #[tokio::test]
    async fn connection_respects_configured_timeout() {
        use std::{io::Read, net::TcpListener};
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/v1", listener.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut bytes = [0; 4096];
            let _ = stream.read(&mut bytes).unwrap();
            std::thread::sleep(Duration::from_millis(1300));
        });
        let error = test_connection(&base, "fixture", "", 1)
            .await
            .err()
            .unwrap();
        assert!(error.contains("1 秒"));
        worker.join().unwrap();
    }

    /// 连接测试使用实际 POST 流程和固定提示词，不访问真实服务、不把短回答判成测智结果。
    #[tokio::test]
    async fn connection_uses_fixed_probe_and_selected_model() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/v1", listener.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut buffer = [0; 4096];
                let count = stream.read(&mut buffer).unwrap();
                assert!(count > 0);
                bytes.extend_from_slice(&buffer[..count]);
                if let Some(split) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..split]);
                    let length: usize = headers
                        .lines()
                        .find_map(|l| {
                            l.to_lowercase()
                                .strip_prefix("content-length: ")
                                .map(String::from)
                        })
                        .unwrap()
                        .parse()
                        .unwrap();
                    if bytes.len() >= split + 4 + length {
                        let payload: Value = serde_json::from_slice(&bytes[split + 4..]).unwrap();
                        assert_eq!(payload["model"], "chosen-fixture");
                        assert_eq!(
                            payload["messages"],
                            serde_json::json!([{"role":"user","content":"请只回复 OK。"}])
                        );
                        assert_eq!(payload["stream"], false);
                        break;
                    }
                }
            }
            let body = r#"{"choices":[{"finish_reason":"stop","message":{"content":"OK"}}]}"#;
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .unwrap();
        });
        assert!(test_connection(&base, "chosen-fixture", "", 60)
            .await
            .is_ok());
        worker.join().unwrap();
    }

    #[test]
    fn models_keep_configured_version_and_deduplicate_catalog() {
        assert_eq!(
            models_endpoint("https://example.com/api/v4/chat/completions")
                .unwrap()
                .as_str(),
            "https://example.com/api/v4/models"
        );
        assert_eq!(
            models_endpoint("http://localhost:1234/v1").unwrap().path(),
            "/v1/models"
        );
        let result = parse_models(br#"{"data":[{"id":"b"},{"id":"a"},{"id":"a"},{"id":""},{"id":"secret"},{"bad":true}],"models":5}"#, "secret").unwrap();
        assert_eq!(
            result.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert!(parse_models(b"{}", "").is_err());
    }

    /// 真实本机 HTTP 检查路径、Authorization、失败不回显服务端密钥；不访问第三方。
    #[tokio::test]
    async fn fetch_uses_models_route_and_never_leaks_error_body() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        for (status, body, success) in [
            ("200 OK", r#"{"data":[{"id":"one"}]}"#, true),
            ("401 Unauthorized", "fixture-secret", false),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let base = format!("http://{}/v1", listener.local_addr().unwrap());
            let worker = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut request = [0; 4096];
                let count = stream.read(&mut request).unwrap();
                let text = String::from_utf8_lossy(&request[..count]).to_lowercase();
                assert!(text.starts_with("get /v1/models "));
                assert!(text.contains("authorization: bearer fixture-secret"));
                stream.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).unwrap();
            });
            let result = fetch_models(&base, "fixture-secret").await;
            assert_eq!(result.is_ok(), success);
            if let Err(error) = result {
                assert!(!error.contains("fixture-secret"));
            }
            worker.join().unwrap();
        }
    }
}
