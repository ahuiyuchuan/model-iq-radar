//! Chat Completions 适配器。只提交一个 user 消息，不提交答案、历史或工具。
//! HTTP 错误只映射状态码，不展示服务商响应体或包含鉴权信息的底层错误。
//! 作者：ahui。

use crate::model::{ModelProfile, TestRule};
use reqwest::{redirect::Policy, Client};
use serde_json::{json, Value};
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use url::Url;

/// 一次请求的结果；传输失败、协议失败及截断都不进入关键词判定。
pub struct Completion {
    pub answer: String,
    pub error: Option<String>,
    pub cancelled: bool,
}

impl Completion {
    fn error(message: &str) -> Self {
        Self {
            answer: String::new(),
            error: Some(message.into()),
            cancelled: false,
        }
    }
}

/// 接受 HTTPS 与本地 HTTP；不允许 URL 中嵌入凭据、查询参数或片段。
pub fn endpoint(base: &str) -> Result<Url, String> {
    let mut url = Url::parse(base.trim()).map_err(|_| "接口地址无效，请填写完整的 HTTPS 地址。")?;
    let local = matches!(
        url.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
    );
    if url.scheme() != "https" && !(url.scheme() == "http" && local) {
        return Err("远程接口必须使用 HTTPS；只有 localhost / 127.0.0.1 / ::1 允许 HTTP。".into());
    }
    if url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("接口地址不能包含用户名、密码、查询参数或片段。".into());
    }
    let path = url.path().trim_end_matches('/');
    if !path.ends_with("/chat/completions") {
        url.set_path(&format!("{path}/chat/completions"));
    }
    Ok(url)
}

/// 忠实解析最终正文；推理字段不参与匹配，缺少结束原因时不能证明回答完整。
pub fn parse_completion(value: &Value) -> Completion {
    let Some(choice) = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
    else {
        return Completion::error("接口未返回 choices，可能不是 Chat Completions 兼容接口。");
    };
    let content = &choice["message"]["content"];
    let answer = if let Some(text) = content.as_str() {
        text.to_string()
    } else if let Some(parts) = content.as_array() {
        parts
            .iter()
            .filter(|part| part["type"] == "text")
            .filter_map(|part| part["text"].as_str())
            .collect::<Vec<_>>()
            .join("")
    } else {
        String::new()
    };
    let error = if choice["finish_reason"] != "stop" {
        Some(
            match choice["finish_reason"].as_str() {
                Some("length") => "回答被截断，未执行判定。",
                Some("content_filter") => "回答被接口过滤，未执行判定。",
                Some("tool_calls" | "function_call") => "模型请求调用工具，本测试不执行工具。",
                _ => "接口未明确返回完整结束状态，未执行判定。",
            }
            .into(),
        )
    } else if answer.trim().is_empty() {
        Some("接口返回空正文，未执行判定。".into())
    } else {
        None
    };
    Completion {
        answer,
        error,
        cancelled: false,
    }
}

/// 请求总体超时包含读取正文，最多接受 2 MiB；取消优先，绝不自动重试。
pub async fn complete(
    profile: &ModelProfile,
    rule: &TestRule,
    key: &str,
    timeout: u64,
    token: &CancellationToken,
) -> Completion {
    let address = match endpoint(&profile.base_url) {
        Ok(value) => value,
        Err(message) => return Completion::error(&message),
    };
    let client = match Client::builder()
        .redirect(Policy::none())
        .connect_timeout(Duration::from_secs(15))
        .build()
    {
        Ok(value) => value,
        Err(_) => return Completion::error("无法初始化 HTTPS 客户端。"),
    };
    let work = async {
        let mut request = client.post(address).json(&json!({
            "model": profile.model,
            "messages": [{"role": "user", "content": rule.prompt}],
            "stream": false
        }));
        if !key.is_empty() {
            request = request.bearer_auth(key);
        }
        let mut response = match request.send().await {
            Ok(value) => value,
            Err(_) => {
                return Completion::error("请求失败，请检查接口地址、网络、代理和 TLS 证书。")
            }
        };
        if !response.status().is_success() {
            let status = response.status().as_u16();
            return Completion::error(&match status {
                401 | 403 => format!("鉴权失败（HTTP {status}），请检查 API Key 和权限。"),
                429 => "请求被限流或额度不足（HTTP 429），请稍后手动重试。".into(),
                300..=399 => "接口返回重定向。为保护密钥，请直接填写最终接口地址。".into(),
                _ => format!("接口返回 HTTP {status}，请检查模型名称和服务状态。"),
            });
        }
        let mut bytes = Vec::new();
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
                        return Completion::error("响应超过 2 MiB，已停止读取，未执行判定。");
                    }
                    bytes.extend_from_slice(&chunk);
                }
                Ok(None) => break,
                Err(_) => return Completion::error("响应读取中断，未执行判定。"),
            }
        }
        match serde_json::from_slice::<Value>(&bytes) {
            Ok(value) => parse_completion(&value),
            Err(_) => Completion::error("响应不是有效的 JSON，未执行判定。"),
        }
    };
    tokio::select! {
        biased;
        _ = token.cancelled() => Completion { answer: String::new(), error: Some("用户已取消。本地停止等待，服务商仍可能已处理请求并计费。".into()), cancelled: true },
        _ = tokio::time::sleep(Duration::from_secs(timeout)) => Completion::error(&format!("请求超过 {timeout} 秒，已停止等待，未执行判定。")),
        result = work => result,
    }
}
