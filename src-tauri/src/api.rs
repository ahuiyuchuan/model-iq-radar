//! Chat Completions 与 Responses 适配器。只提交一个独立题目，不提交答案、历史或工具。
//! HTTP 错误只映射状态码，不展示服务商响应体或包含鉴权信息的底层错误。
//! 正式 Responses 测试的 JSON/读取异常可独立落盘本次响应取证，不参与判定或历史正文。
//! 作者：ahui。

use crate::{
    model::{validate_timeout_seconds, ModelProfile, TestRule},
    response_diagnostics::{self, JsonParseError, ResponseDiagnostic, BODY_LIMIT_BYTES},
    responses_sse,
};
use reqwest::{header::HeaderMap, redirect::Policy, Client};
use serde_json::{json, Value};
use std::{path::Path, time::Duration};
use tokio_util::sync::CancellationToken;
use url::Url;

/// 一次请求的结果；传输失败、协议失败及截断都不进入关键词判定。
pub struct Completion {
    pub answer: String,
    pub error: Option<String>,
    pub cancelled: bool,
    /// 历史/UI 只保留安全元数据；独立响应诊断不写入历史，不保存请求鉴权头。
    pub error_kind: Option<String>,
    pub http_status: Option<u16>,
    pub request_id: Option<String>,
    /// 首选协议及可选回退协议的真实尝试顺序；请求前本地失败时为空。
    pub attempted_modes: Vec<String>,
}

impl Completion {
    /// 构造不会泄露原始网络错误的失败；供请求前凭据校验等调用方统一使用。
    pub fn failure(kind: &str, message: &str) -> Self {
        Self {
            answer: String::new(),
            error: Some(message.into()),
            cancelled: false,
            error_kind: Some(kind.into()),
            http_status: None,
            request_id: None,
            attempted_modes: Vec::new(),
        }
    }

    fn error(message: &str) -> Self {
        Self::failure("protocol", message)
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
    // 配置可填写另一个协议的完整地址，切换模式时替换末段，不能叠加两个端点。
    let path = url.path().trim_end_matches('/');
    let path = path.strip_suffix("/responses").unwrap_or(path);
    if !path.ends_with("/chat/completions") {
        url.set_path(&format!("{path}/chat/completions"));
    } else {
        let path = path.to_string();
        url.set_path(&path);
    }
    Ok(url)
}

/// 将配置地址规范化为 Responses 端点；旧的完整 Chat Completions 地址会替换末段。
pub fn responses_endpoint(base: &str) -> Result<Url, String> {
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
    if let Some(root) = path.strip_suffix("/chat/completions") {
        url.set_path(&format!("{root}/responses"));
    } else if !path.ends_with("/responses") {
        url.set_path(&format!("{path}/responses"));
    } else {
        let path = path.to_string();
        url.set_path(&path);
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
    let message = &choice["message"];
    // 标准协议的正文必须来自 assistant；兼容旧网关省略 role 的情况，
    // 但不能把显式 user/tool 消息或 refusal 与普通回答混合后执行关键词判定。
    let invalid_role = message
        .get("role")
        .is_some_and(|role| !role.is_null() && role != "assistant");
    let has_refusal = message
        .get("refusal")
        .is_some_and(|refusal| !refusal.is_null());
    let has_tools = message.get("function_call").is_some_and(|v| !v.is_null())
        || message
            .get("tool_calls")
            .is_some_and(|v| !v.is_null() && v.as_array().is_none_or(|items| !items.is_empty()));
    // 某些网关错误地在混合工具响应上返回 stop，仍不能据此判定普通正文。
    let unsupported_parts = content.as_array().is_some_and(|parts| {
        parts.iter().any(|part| {
            part["type"] != "text" || part.get("text").and_then(Value::as_str).is_none()
        })
    });
    let failure = if invalid_role {
        Some(("protocol", "接口返回非助手消息，未执行判定。"))
    } else if has_refusal {
        Some(("filtered", "接口返回拒绝回答，未执行判定。"))
    } else if has_tools {
        Some(("tool_call", "模型请求调用工具，本测试不执行工具。"))
    } else if unsupported_parts {
        Some(("protocol", "接口返回不支持的正文类型，未执行判定。"))
    } else if choice["finish_reason"] != "stop" {
        Some(match choice["finish_reason"].as_str() {
            Some("length") => ("incomplete", "回答被截断，未执行判定。"),
            Some("content_filter") => ("filtered", "回答被接口过滤，未执行判定。"),
            Some("tool_calls" | "function_call") => {
                ("tool_call", "模型请求调用工具，本测试不执行工具。")
            }
            _ => ("protocol", "接口未明确返回完整结束状态，未执行判定。"),
        })
    } else if answer.trim().is_empty() {
        Some(("empty_response", "接口返回空正文，未执行判定。"))
    } else {
        None
    };
    Completion {
        answer,
        error_kind: failure.map(|(kind, _)| kind.into()),
        error: failure.map(|(_, message)| message.into()),
        cancelled: false,
        http_status: None,
        request_id: None,
        attempted_modes: Vec::new(),
    }
}

/// 解析 Responses 的最终 output 消息；推理内容不参与匹配。
/// 顶层和 message 都须明确完成，工具调用或未知输出不能与正文混合判定。
pub fn parse_response(value: &Value) -> Completion {
    if value.get("status").and_then(Value::as_str) != Some("completed") {
        return Completion::failure(
            "incomplete",
            match value["status"].as_str() {
                Some("incomplete") => "Responses 回答未完整结束，未执行判定。",
                Some("failed") => "Responses 请求失败，未执行判定。",
                _ => "Responses 未返回完整结束状态，未执行判定。",
            },
        );
    }
    let Some(items) = value.get("output").and_then(Value::as_array) else {
        return Completion::error("Responses 接口未返回 output 消息，未执行判定。");
    };
    let mut chunks = Vec::new();
    for item in items {
        match item.get("type").and_then(Value::as_str) {
            Some("reasoning") => continue,
            Some("message") if item["status"] == "completed" && item["role"] == "assistant" => {
                // Responses 的 phase 区分中间说明和最终回答。旧接口未提供 phase
                // 时保持既有兼容；明确 commentary 不能污染完整最终回答的字符串判定。
                // 官方契约：https://developers.openai.com/api/reference/resources/responses/methods/create
                let is_final = match item.get("phase") {
                    None | Some(Value::Null) => true,
                    Some(Value::String(phase)) if phase == "final_answer" => true,
                    Some(Value::String(phase)) if phase == "commentary" => false,
                    _ => {
                        return Completion::error(
                            "Responses 消息阶段无效，无法确认最终正文，未执行判定。",
                        )
                    }
                };
                let Some(content) = item.get("content").and_then(Value::as_array) else {
                    return Completion::error("Responses 消息缺少正文，未执行判定。");
                };
                for part in content {
                    if part["type"] == "output_text" {
                        let Some(text) = part.get("text").and_then(Value::as_str) else {
                            return Completion::error("Responses 正文格式无效，未执行判定。");
                        };
                        if is_final {
                            chunks.push(text);
                        }
                    } else if part["type"] == "refusal" {
                        return Completion::failure(
                            "filtered",
                            "Responses 返回拒绝回答，未执行判定。",
                        );
                    } else {
                        return Completion::error("Responses 返回不支持的正文类型，未执行判定。");
                    }
                }
            }
            Some("message") => {
                return Completion::failure("incomplete", "Responses 消息未明确完成，未执行判定。")
            }
            _ => {
                return Completion::failure(
                    "tool_call",
                    "Responses 包含工具调用或未知输出，本测试不执行工具。",
                )
            }
        }
    }
    let answer = chunks.concat();
    if answer.trim().is_empty() {
        return Completion::failure(
            "empty_response",
            "Responses 接口未返回有效正文，未执行判定。",
        );
    }
    Completion {
        answer,
        error: None,
        cancelled: false,
        error_kind: None,
        http_status: None,
        request_id: None,
        attempted_modes: Vec::new(),
    }
}

/// 不信任服务商请求编号，只接受常见 ASCII 编号字符及最多 200 字节。
/// 不保存其他响应头，避免将 cookie、令牌或多行内容带入可复制诊断。
fn safe_request_id(headers: &HeaderMap, key: &str) -> Option<String> {
    [
        "x-request-id",
        "request-id",
        "x-amzn-requestid",
        "x-ms-request-id",
    ]
    .into_iter()
    .find_map(|name| {
        let value = headers.get(name)?.to_str().ok()?;
        if value.is_empty()
            || value.len() > 200
            || (!key.is_empty() && value.contains(key))
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:/".contains(&byte))
        {
            None
        } else {
            Some(value.into())
        }
    })
}

/// 单次测试最多发送两次请求：只有显式启用且首选协议请求错误时尝试另一协议。
/// 两次共用开始时确定的总期限与取消令牌；正常回答判定失败不属于请求错误。
/// 首选配置不回写，最终正文和错误只来自最后一次尝试，历史额外保留尝试顺序。
pub async fn complete(
    profile: &ModelProfile,
    rule: &TestRule,
    key: &str,
    timeout: u64,
    token: &CancellationToken,
) -> Completion {
    complete_with_diagnostics(profile, rule, key, timeout, token, None).await
}

/// 正式测试的可选本地响应取证入口；None 不写文件，已有调用方沿用 complete。
/// 仅 Responses 的 JSON/SSE/正文读取异常保存已收到字节；不额外请求、不启用回退、不改变成功标准。
/// 文件后台写入不计入网络预算、不等待磁盘；读取到 EOF 才可标记完整，超限只保留有界前缀。
pub async fn complete_with_diagnostics(
    profile: &ModelProfile,
    rule: &TestRule,
    key: &str,
    timeout: u64,
    token: &CancellationToken,
    diagnostic_directory: Option<&Path>,
) -> Completion {
    if let Err(message) = validate_timeout_seconds(timeout) {
        return Completion::failure("configuration", &message);
    }
    // 用户可自定正整数秒，极大值不能让时钟加法溢出并使运行任务异常退出。
    // 计时器无法表示时在发请求前报错，不截断、不改用默认值，也不尝试协议回退。
    let Some(deadline) = tokio::time::Instant::now().checked_add(Duration::from_secs(timeout))
    else {
        return Completion::failure(
            "configuration",
            "超时超过系统计时器可表示的范围，请减小秒数。",
        );
    };
    let mut result = complete_once(
        profile,
        rule,
        key,
        timeout,
        token,
        deadline,
        diagnostic_directory,
    )
    .await;
    if result.error_kind.as_deref() == Some("configuration") {
        return result;
    }
    result.attempted_modes.push(profile.interface_mode.clone());
    if !profile.protocol_fallback_enabled
        || result.error.is_none()
        || result.cancelled
        || token.is_cancelled()
        || tokio::time::Instant::now() >= deadline
    {
        return result;
    }
    let mut fallback = profile.clone();
    fallback.interface_mode = if profile.interface_mode == "responses" {
        "chat_completions"
    } else {
        "responses"
    }
    .into();
    let mut retried = complete_once(
        &fallback,
        rule,
        key,
        timeout,
        token,
        deadline,
        diagnostic_directory,
    )
    .await;
    result.attempted_modes.push(fallback.interface_mode);
    retried.attempted_modes = result.attempted_modes;
    retried
}

/// 仅执行所选协议一次，不递归回退；总超时包含响应读取，单次正文上限 2 MiB。
async fn complete_once(
    profile: &ModelProfile,
    rule: &TestRule,
    key: &str,
    timeout: u64,
    token: &CancellationToken,
    deadline: tokio::time::Instant,
    diagnostic_directory: Option<&Path>,
) -> Completion {
    if !matches!(
        profile.interface_mode.as_str(),
        "responses" | "chat_completions"
    ) {
        return Completion::failure("configuration", "接口模式无效，请重新选择。");
    }
    let address = match if profile.interface_mode == "responses" {
        responses_endpoint(&profile.base_url)
    } else {
        endpoint(&profile.base_url)
    } {
        Ok(value) => value,
        Err(message) => return Completion::failure("configuration", &message),
    };
    let client = match Client::builder()
        .redirect(Policy::none())
        .connect_timeout(Duration::from_secs(15))
        .build()
    {
        Ok(value) => value,
        Err(_) => return Completion::failure("configuration", "无法初始化 HTTPS 客户端。"),
    };
    // 正文缓冲在 select 外持有，取消/超时丢弃网络 future 后仍能保存已经收到的部分。
    // 不为诊断读取更多字节；与原来一样，超过既有 2 MiB 限制即停止并返回原错误类型。
    let mut bytes = Vec::new();
    let mut received_body_bytes = 0_u64;
    let mut body_complete = false;
    let mut body_truncated = false;
    let mut json_error = None;
    let mut response_metadata = None;
    let mut sse_response = false;
    let started_at_utc = chrono::Utc::now().to_rfc3339();
    let work = async {
        let mut payload = if profile.interface_mode == "responses" {
            json!({
                "model": profile.model,
                "input": rule.prompt,
                "stream": false
            })
        } else {
            json!({
                "model": profile.model,
                "messages": [{"role": "user", "content": rule.prompt}],
                "stream": false
            })
        };
        if let Some(effort) = profile
            .reasoning_effort
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            if profile.interface_mode == "responses" {
                payload["reasoning"] = json!({"effort": effort});
            } else {
                payload["reasoning_effort"] = json!(effort);
            }
        }
        let mut request = client.post(address).json(&payload);
        if !key.is_empty() {
            request = request.bearer_auth(key);
        }
        let mut response = match request.send().await {
            Ok(value) => value,
            Err(error) => {
                return Completion::failure(
                    if error.is_timeout() {
                        "timeout"
                    } else {
                        "connection"
                    },
                    "请求失败，请检查接口地址、网络、代理和 TLS 证书。",
                )
            }
        };
        let status = response.status().as_u16();
        let request_id = safe_request_id(response.headers(), key);
        let with_metadata = |mut result: Completion| {
            result.http_status = Some(status);
            result.request_id = request_id.clone();
            result
        };
        if !response.status().is_success() {
            let (kind, message) = match status {
                401 | 403 => ("authentication", format!("鉴权失败（HTTP {status}），请检查 API Key 和权限。")),
                429 => ("rate_limit", "请求被限流或额度不足（HTTP 429），请稍后手动重试。".into()),
                300..=399 => ("redirect", "接口返回重定向。为保护密钥，请直接填写最终接口地址。".into()),
                400 | 422 if profile.reasoning_effort.is_some() => ("request_rejected", format!("接口拒绝请求（HTTP {status}），请核对模型、接口模式及思考强度支持范围；不支持思考强度时请选择“不指定”。未自动换档。")),
                _ => ("http", format!("接口返回 HTTP {status}，请检查模型名称和服务状态。")),
            };
            return with_metadata(Completion::failure(kind, &message));
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok());
        sse_response = profile.interface_mode == "responses"
            && responses_sse::is_event_stream(content_type, &[]);
        // 只保留所需的响应头；不复制请求对象、鉴权头、请求体、URL 或其他响应头。
        if diagnostic_directory.is_some() && profile.interface_mode == "responses" {
            let content_type = content_type
                .filter(|value| value.len() <= 256 && (key.is_empty() || !value.contains(key)))
                .map(str::to_owned);
            response_metadata = Some((status, content_type, request_id.clone()));
        }
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    received_body_bytes = received_body_bytes.saturating_add(chunk.len() as u64);
                    let remaining = BODY_LIMIT_BYTES.saturating_sub(bytes.len());
                    bytes.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
                    if chunk.len() > remaining {
                        body_truncated = true;
                        return with_metadata(Completion::failure(
                            "response_too_large",
                            "响应超过 2 MiB，已停止读取，未执行判定。",
                        ));
                    }
                }
                Ok(None) => {
                    body_complete = true;
                    break;
                }
                Err(_) => {
                    return with_metadata(Completion::failure(
                        "read_interrupted",
                        "响应读取中断，未执行判定。",
                    ))
                }
            }
        }
        // 仍先读取到 EOF：中断、超时或超限不能靠已收到的增量/结束片段绕过完整性检查。
        // 有些中转即使收到 stream:false 也返回 SSE；只改变本次正文解析，不重发请求。
        sse_response = profile.interface_mode == "responses"
            && (sse_response || responses_sse::is_event_stream(None, &bytes));
        // 完整 JSON 始终保留原兼容路径，即使网关误标 Content-Type 也不无故破坏旧行为。
        let parsed_json = serde_json::from_slice::<Value>(&bytes);
        if sse_response && parsed_json.is_err() {
            return with_metadata(match responses_sse::parse(&bytes) {
                Ok(value) => parse_response(&value),
                Err(failure) => {
                    if let Some(error) = failure.json_error {
                        json_error = Some(JsonParseError {
                            message: error.to_string(),
                            line: error.line(),
                            column: error.column(),
                            scope: "sse_event_data",
                            event_index: Some(failure.event_index),
                        });
                    }
                    Completion::failure(failure.kind, failure.message)
                }
            });
        }
        with_metadata(match parsed_json {
            Ok(value) => {
                if profile.interface_mode == "responses" {
                    parse_response(&value)
                } else {
                    parse_completion(&value)
                }
            }
            Err(error) => {
                json_error = Some(JsonParseError {
                    message: error.to_string(),
                    line: error.line(),
                    column: error.column(),
                    scope: "response_body",
                    event_index: None,
                });
                Completion::failure("invalid_json", "响应不是有效的 JSON，未执行判定。")
            }
        })
    };
    let result = tokio::select! {
        biased;
        _ = token.cancelled() => { let mut result=Completion::failure("cancelled", "用户已取消。本地停止等待，服务商仍可能已处理请求并计费。"); result.cancelled=true; result },
        _ = tokio::time::sleep_until(deadline) => Completion::failure("timeout", &format!("请求超过 {timeout} 秒，已停止等待，未执行判定。")),
        result = work => result,
    };
    // SSE 解析或最终协议验证失败同样保留原始事件流；正常成功不新增诊断文件。
    if (sse_response && result.error.is_some())
        || matches!(
            result.error_kind.as_deref(),
            Some(
                "invalid_json"
                    | "response_too_large"
                    | "read_interrupted"
                    | "timeout"
                    | "cancelled"
            )
        )
    {
        if let (Some(directory), Some((status, content_type, request_id))) =
            (diagnostic_directory, response_metadata)
        {
            response_diagnostics::save_in_background(
                directory,
                ResponseDiagnostic {
                    format_version: 2,
                    started_at_utc,
                    finished_at_utc: chrono::Utc::now().to_rfc3339(),
                    protocol: "responses",
                    http_status: status,
                    content_type,
                    request_id,
                    error_kind: result.error_kind.clone().unwrap_or_default(),
                    received_body_bytes,
                    saved_body_bytes: bytes.len(),
                    body_limit_bytes: BODY_LIMIT_BYTES,
                    body_complete,
                    body_truncated,
                    json_error,
                    body_file: None,
                    body_omission_reason: None,
                    body: bytes,
                },
                key,
            );
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    /// 端点模式互换应保持服务根路径，尤其不能产生 responses/chat/completions。
    #[test]
    fn switching_protocol_replaces_complete_endpoint() {
        assert_eq!(
            endpoint("https://example.com/api/v1/responses/")
                .unwrap()
                .as_str(),
            "https://example.com/api/v1/chat/completions"
        );
        assert_eq!(
            responses_endpoint("https://example.com/api/v1/chat/completions/")
                .unwrap()
                .as_str(),
            "https://example.com/api/v1/responses"
        );
    }

    /// 缺状态、未完成消息、工具混合和只有推理的响应都不能误判通过。
    #[test]
    fn responses_requires_complete_assistant_message_without_tools() {
        let message = json!({"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"  高市早苗\n"}]});
        let valid = json!({"status":"completed","output":[{"type":"reasoning","summary":[{"text":"不参与判定"}]},message]});
        assert_eq!(parse_response(&valid).answer, "  高市早苗\n");
        let mut invalid = valid.clone();
        invalid.as_object_mut().unwrap().remove("status");
        assert!(parse_response(&invalid).error.is_some());
        invalid = valid.clone();
        invalid["output"][1]["status"] = json!("in_progress");
        assert!(parse_response(&invalid).error.is_some());
        invalid = valid.clone();
        invalid["output"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type":"function_call","name":"search"}));
        assert_eq!(
            parse_response(&invalid).error_kind.as_deref(),
            Some("tool_call")
        );
        assert!(parse_response(
            &json!({"status":"completed","output":[{"type":"reasoning"}],"output_text":"高市早苗"})
        )
        .error
        .is_some());
        assert!(parse_completion(&json!({"choices":[{"finish_reason":"stop","message":{"content":"高市早苗","tool_calls":[{"id":"tool"}]}}]})).error.is_some());
    }

    /// 中间说明包含关键词不能令最终答案误通过；旧接口省略 phase 仍可读取。
    #[test]
    fn responses_excludes_commentary_and_rejects_unknown_message_phases() {
        let commentary = json!({"type":"message","role":"assistant","status":"completed","phase":"commentary","content":[{"type":"output_text","text":"我将核对高市早苗。"}]});
        let final_answer = json!({"type":"message","role":"assistant","status":"completed","phase":"final_answer","content":[{"type":"output_text","text":"  旧答案\n"}]});
        let mut value = json!({"status":"completed","output":[commentary, final_answer]});
        let parsed = parse_response(&value);
        assert!(parsed.error.is_none());
        assert_eq!(parsed.answer, "  旧答案\n");
        assert!(!parsed.answer.contains("高市早苗"));

        value["output"].as_array_mut().unwrap().pop();
        assert_eq!(
            parse_response(&value).error_kind.as_deref(),
            Some("empty_response")
        );
        value["output"][0]["phase"] = json!("unknown_phase");
        assert_eq!(
            parse_response(&value).error_kind.as_deref(),
            Some("protocol")
        );
    }

    /// 兼容网关可以省略 role，但显式非助手消息和 refusal 不能被误当最终答案。
    #[test]
    fn chat_rejects_non_assistant_and_refusal_mixed_with_text() {
        let mut value =
            json!({"choices":[{"finish_reason":"stop","message":{"content":"高市早苗"}}]});
        assert!(parse_completion(&value).error.is_none());
        value["choices"][0]["message"]["role"] = json!("user");
        assert_eq!(
            parse_completion(&value).error_kind.as_deref(),
            Some("protocol")
        );
        value["choices"][0]["message"]["role"] = json!("assistant");
        value["choices"][0]["message"]["refusal"] = json!("无法回答");
        assert_eq!(
            parse_completion(&value).error_kind.as_deref(),
            Some("filtered")
        );
        value["choices"][0]["message"]["refusal"] = Value::Null;
        assert!(parse_completion(&value).error.is_none());
    }

    /// 请求编号只保存可展示的有限 ASCII 元数据，不能回显服务端夹带的密钥。
    #[test]
    fn request_identifiers_filter_untrusted_or_secret_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-request-id", HeaderValue::from_static("req-123_ABC"));
        assert_eq!(
            safe_request_id(&headers, "secret"),
            Some("req-123_ABC".into())
        );
        headers.insert("x-request-id", HeaderValue::from_static("echo-secret"));
        assert!(safe_request_id(&headers, "secret").is_none());
        headers.insert(
            "x-request-id",
            HeaderValue::from_static("<script>alert(1)</script>"),
        );
        assert!(safe_request_id(&headers, "").is_none());
    }
}
