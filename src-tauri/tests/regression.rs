//! 核心边界回归：请求隔离、完整性、取消、大小限制和 SQLite 规则快照。
//! 仅使用本机随机端口与临时数据库，不访问第三方模型。作者：ahui。

use model_probe_lib::{
    api,
    model::{AppConfig, ModelProfile, TestRecord, TestRule},
    storage::Store,
};
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::mpsc,
    thread,
    time::Duration,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// 本地一次性 HTTP 服务，用于观测真实请求而不是复制客户端组包实现。
fn fixture(status: &str, response: String, delay: Duration) -> (String, mpsc::Receiver<String>) {
    sequence_fixture(vec![(status.into(), response, delay)])
}

/// 同一服务顺序返回不同协议的响应，观测真实重试次数与每次组包；仅监听本机随机端口。
fn sequence_fixture(
    responses: Vec<(String, String, Duration)>,
) -> (String, mpsc::Receiver<String>) {
    wire_fixture(responses.into_iter().map(|(status, response, delay)| {
        (format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}", response.len()), delay, Duration::ZERO)
    }).collect())
}

/// 原始 HTTP 模拟可指定响应头、错误长度和延迟 EOF；验证传输完整性，不复制客户端解析器。
fn wire_fixture(responses: Vec<(String, Duration, Duration)>) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = format!("http://{}/v1", listener.local_addr().unwrap());
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for (response, delay, hold_open) in responses {
            // 未发生预期回退（例如取消/超时）时也应回收服务线程，不永久阻塞 accept。
            let until = std::time::Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if std::time::Instant::now() >= until {
                            return;
                        }
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("本机测试服务 accept 失败：{error}"),
                }
            };
            // Windows 的已接收套接字可能继承监听器的非阻塞模式；正文仍使用有界阻塞读取。
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0; 4096];
            loop {
                let count = stream.read(&mut chunk).unwrap();
                if count == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..count]);
                if let Some(split) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..split]);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|value| value.parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= split + 4 + length {
                        break;
                    }
                }
            }
            let _ = sender.send(String::from_utf8(bytes).unwrap());
            thread::sleep(delay);
            let _ = stream.write_all(response.as_bytes());
            thread::sleep(hold_open);
        }
    });
    (address, receiver)
}

/// 合成 Responses SSE，最终正文与增量故意不同，确保不能拼接增量替代终态。
fn synthetic_sse() -> String {
    format!(": keepalive\r\n\r\ndata: {{\"type\":\"response.output_text.delta\",\"delta\":\"错误增量\"}}\r\n\r\nevent: response.completed\r\ndata: {}\r\n\r\n", json!({
        "type":"response.completed", "response":{"id":"resp_fixture", "status":"completed", "output":[{"type":"message","role":"assistant","phase":"final_answer","status":"completed","content":[{"type":"output_text","text":"21颗"}]}]}
    }))
}

fn http_body(body: &str, mime: &str) -> String {
    format!("HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nx-request-id: fixture-request\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
}

/// 仅测试等待后台元数据写完；最多三秒，不把文件延迟计入生产测试的请求期限。
async fn wait_for_diagnostic(directory: &std::path::Path) -> serde_json::Value {
    let until = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        if let Ok(entries) = std::fs::read_dir(directory) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if path.extension().is_some_and(|v| v == "json") {
                    if let Ok(bytes) = std::fs::read(path) {
                        if let Ok(metadata) = serde_json::from_slice(&bytes) {
                            return metadata;
                        }
                    }
                }
            }
        }
        assert!(std::time::Instant::now() < until, "诊断文件未及时完成");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// 正确/错误 MIME 均兼容一次 SSE 响应；开着回退也不能因解析方式多发一次请求。
#[tokio::test]
async fn responses_sse_single_request_preserves_stream_false_and_protocol_choice() {
    for mime in ["text/event-stream; charset=utf-8", "application/json"] {
        let (base, requests) = wire_fixture(vec![(
            http_body(&synthetic_sse(), mime),
            Duration::ZERO,
            Duration::ZERO,
        )]);
        let mut selected = profile(base);
        selected.interface_mode = "responses".into();
        selected.protocol_fallback_enabled = true;
        let result = api::complete(
            &selected,
            &TestRule::default(),
            "",
            10,
            &CancellationToken::new(),
        )
        .await;
        assert!(result.error.is_none(), "{:?}", result.error);
        assert_eq!(result.answer, "21颗");
        assert_eq!(result.attempted_modes, ["responses"]);
        assert_eq!(result.http_status, Some(200));
        assert_eq!(result.request_id.as_deref(), Some("fixture-request"));
        let request = requests.recv_timeout(Duration::from_secs(2)).unwrap();
        let payload: serde_json::Value =
            serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(payload["stream"], false);
        assert!(request.starts_with("POST /v1/responses "));
        assert!(requests.try_recv().is_err());
    }
}

#[tokio::test]
async fn valid_json_wins_wrong_sse_mime_and_chat_never_parses_sse() {
    let body = json!({"status":"completed","output":[{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"原始 JSON"}]}]}).to_string();
    let (base, _) = wire_fixture(vec![(
        http_body(&body, "text/event-stream"),
        Duration::ZERO,
        Duration::ZERO,
    )]);
    let mut selected = profile(base);
    selected.interface_mode = "responses".into();
    let result = api::complete(
        &selected,
        &TestRule::default(),
        "",
        10,
        &CancellationToken::new(),
    )
    .await;
    assert_eq!(result.answer, "原始 JSON");
    let (base, _) = wire_fixture(vec![(
        http_body(&synthetic_sse(), "text/event-stream"),
        Duration::ZERO,
        Duration::ZERO,
    )]);
    let result = api::complete(
        &profile(base),
        &TestRule::default(),
        "",
        10,
        &CancellationToken::new(),
    )
    .await;
    assert_eq!(result.error_kind.as_deref(), Some("invalid_json"));
}

/// 即使正文含 completed，也必须读完本次 HTTP；错误长度、超限、超时与取消不能成功。
#[tokio::test]
async fn sse_transport_boundaries_never_accept_partial_or_oversized_body() {
    let stream = synthetic_sse();
    let interrupted = format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{stream}", stream.len() + 100);
    let oversized = format!("{stream}:{}\n\n", "x".repeat(2 * 1024 * 1024));
    for (wire, hold_open, timeout, cancel, expected) in [
        (interrupted, Duration::ZERO, 10, false, "read_interrupted"),
        (http_body(&oversized, "text/event-stream"), Duration::ZERO, 10, false, "response_too_large"),
        (format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{stream}"), Duration::from_secs(2), 1, false, "timeout"),
        (format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{stream}"), Duration::from_secs(2), 10, true, "cancelled"),
    ] {
        let (base, _) = wire_fixture(vec![(wire, Duration::ZERO, hold_open)]);
        let mut selected = profile(base);
        selected.interface_mode = "responses".into();
        let token = CancellationToken::new();
        if cancel {
            let canceller = token.clone();
            tokio::spawn(async move { tokio::time::sleep(Duration::from_millis(100)).await; canceller.cancel(); });
        }
        let result = api::complete(&selected, &TestRule::default(), "", timeout, &token).await;
        assert_eq!(result.error_kind.as_deref(), Some(expected));
        assert!(result.answer.is_empty());
        assert_eq!(result.attempted_modes, ["responses"]);
    }
}

/// 独立临时目录验证真实请求到后台文件的连接；既不访问用户日志，也不请求真实服务。
#[tokio::test]
async fn response_diagnostics_capture_original_bytes_and_do_not_poison_next_request() {
    let directory = std::env::temp_dir().join(format!("probe-http-diagnostics-{}", Uuid::new_v4()));
    let body = "data: {\r\ndata: \"type\":\r\n\r\n";
    let (base, requests) = wire_fixture(vec![
        (
            http_body(body, "text/event-stream"),
            Duration::ZERO,
            Duration::ZERO,
        ),
        (
            http_body(&synthetic_sse(), "text/event-stream"),
            Duration::ZERO,
            Duration::ZERO,
        ),
    ]);
    let mut selected = profile(base);
    selected.interface_mode = "responses".into();
    let result = api::complete_with_diagnostics(
        &selected,
        &TestRule::default(),
        "fixture-key",
        10,
        &CancellationToken::new(),
        Some(&directory),
    )
    .await;
    assert_eq!(result.error_kind.as_deref(), Some("invalid_json"));
    assert_eq!(result.attempted_modes, ["responses"]);
    assert!(result.answer.is_empty());
    let metadata = wait_for_diagnostic(&directory).await;
    assert_eq!(metadata["http_status"], 200);
    assert_eq!(metadata["request_id"], "fixture-request");
    assert_eq!(metadata["body_complete"], true);
    assert_eq!(metadata["body_truncated"], false);
    assert_eq!(metadata["received_body_bytes"], body.len());
    assert_eq!(metadata["json_error"]["scope"], "sse_event_data");
    assert_eq!(metadata["json_error"]["event_index"], 1);
    assert_eq!(
        std::fs::read(directory.join(metadata["body_file"].as_str().unwrap())).unwrap(),
        body.as_bytes()
    );
    let result = api::complete_with_diagnostics(
        &selected,
        &TestRule::default(),
        "fixture-key",
        10,
        &CancellationToken::new(),
        Some(&directory),
    )
    .await;
    assert!(result.error.is_none());
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 2);
    assert_eq!(requests.try_iter().count(), 2);
    std::fs::remove_dir_all(directory).unwrap();
}

/// 完整语法错误、短读与超限的标志必须不同；保存量与原始字节均可核对，不能伪装完整正文。
#[tokio::test]
async fn diagnostic_limits_and_read_completeness_are_explicit() {
    let large = "x".repeat(2 * 1024 * 1024 + 1);
    for (body, declared_length, expected, complete, truncated) in [
        ("{broken".to_owned(), 7, "invalid_json", true, false),
        ("{partial".to_owned(), 100, "read_interrupted", false, false),
        (
            large.clone(),
            large.len(),
            "response_too_large",
            false,
            true,
        ),
    ] {
        let directory = std::env::temp_dir().join(format!("probe-http-limit-{}", Uuid::new_v4()));
        let wire = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {declared_length}\r\nConnection: close\r\n\r\n{body}");
        let (base, requests) = wire_fixture(vec![(wire, Duration::ZERO, Duration::ZERO)]);
        let mut selected = profile(base);
        selected.interface_mode = "responses".into();
        let result = api::complete_with_diagnostics(
            &selected,
            &TestRule::default(),
            "",
            10,
            &CancellationToken::new(),
            Some(&directory),
        )
        .await;
        assert_eq!(result.error_kind.as_deref(), Some(expected));
        assert!(result.answer.is_empty());
        let metadata = wait_for_diagnostic(&directory).await;
        assert_eq!(metadata["body_complete"], complete);
        assert_eq!(metadata["body_truncated"], truncated);
        let saved = std::fs::read(directory.join(metadata["body_file"].as_str().unwrap())).unwrap();
        assert_eq!(saved, body.as_bytes()[..body.len().min(2 * 1024 * 1024)]);
        assert_eq!(metadata["saved_body_bytes"], saved.len());
        if expected == "invalid_json" {
            assert_eq!(metadata["json_error"]["scope"], "response_body");
            assert_eq!(metadata["json_error"]["line"], 1);
        } else {
            assert!(metadata["json_error"].is_null());
        }
        assert_eq!(requests.try_iter().count(), 1);
        std::fs::remove_dir_all(directory).unwrap();
    }
}

fn profile(base_url: String) -> ModelProfile {
    ModelProfile {
        id: "fixture".into(),
        name: "本地测试".into(),
        base_url,
        model: "fixture-model".into(),
        has_key: false,
        credential_revision: None,
        interface_mode: "chat_completions".into(),
        protocol_fallback_enabled: false,
        reasoning_effort: Some("low".into()),
        timeout_seconds: None,
        rule_preset_id: None,
        catalog_token: None,
    }
}

#[test]
fn endpoint_validation_and_normalization() {
    assert_eq!(
        api::endpoint("https://example.com/v1/").unwrap().as_str(),
        "https://example.com/v1/chat/completions"
    );
    assert_eq!(
        api::endpoint("https://example.com/v1/chat/completions")
            .unwrap()
            .path(),
        "/v1/chat/completions"
    );
    assert!(api::endpoint("http://remote.example/v1").is_err());
    assert!(api::endpoint("https://user:password@example.com/v1").is_err());
    assert!(api::endpoint("https://example.com/v1?key=secret").is_err());
    assert!(api::endpoint("http://[::1]:1234/v1").is_ok());
    assert_eq!(
        api::responses_endpoint("https://example.com/v1")
            .unwrap()
            .as_str(),
        "https://example.com/v1/responses"
    );
    assert_eq!(
        api::responses_endpoint("https://example.com/v1/chat/completions")
            .unwrap()
            .as_str(),
        "https://example.com/v1/responses"
    );
}

/// 真实 HTTP 验证双向切换、完整端点替换、协议组包与思考参数；不发送判定答案。
#[tokio::test]
async fn explicit_fallback_uses_other_protocol_once_without_rewriting_profile() {
    let chat = json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":"完成"}}]}).to_string();
    let responses = json!({"status":"completed","output":[{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"完成"}]}]}).to_string();
    for (primary, alternate, success) in [
        ("chat_completions", "responses", responses),
        ("responses", "chat_completions", chat),
    ] {
        let (base, requests) = sequence_fixture(vec![
            ("404 Not Found".into(), "{}".into(), Duration::ZERO),
            ("200 OK".into(), success, Duration::ZERO),
        ]);
        let mut selected = profile(format!(
            "{base}/{}",
            if primary == "responses" {
                "responses"
            } else {
                "chat/completions"
            }
        ));
        selected.interface_mode = primary.into();
        selected.protocol_fallback_enabled = true;
        let rule = TestRule {
            prompt: "独立问题".into(),
            keyword: "do-not-send-secret-answer".into(),
            alternative_keywords: Vec::new(),
            timeout_seconds: None,
        };
        let result = api::complete(
            &selected,
            &rule,
            "fixture-secret",
            10,
            &CancellationToken::new(),
        )
        .await;
        assert!(result.error.is_none(), "{:?}", result.error);
        assert_eq!(result.attempted_modes, [primary, alternate]);
        assert_eq!(selected.interface_mode, primary);
        for mode in [primary, alternate] {
            let request = requests.recv_timeout(Duration::from_secs(2)).unwrap();
            let suffix = if mode == "responses" {
                "responses"
            } else {
                "chat/completions"
            };
            assert!(request.starts_with(&format!("POST /v1/{suffix} ")));
            let body: serde_json::Value =
                serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
            assert_eq!(body["model"], "fixture-model");
            assert!(!request.contains("do-not-send-secret-answer"));
            if mode == "responses" {
                assert_eq!(body["input"], "独立问题");
                assert_eq!(body["reasoning"]["effort"], "low");
                assert!(body.get("messages").is_none());
            } else {
                assert_eq!(body["messages"][0]["content"], "独立问题");
                assert_eq!(body["reasoning_effort"], "low");
                assert!(body.get("input").is_none());
            }
        }
        assert!(requests.try_recv().is_err());
    }
}

/// 关闭开关仅一次；正常答错不能当作传输失败重测；两协议失败也不继续往返重试。
#[tokio::test]
async fn fallback_obeys_disabled_success_and_two_attempt_limit() {
    for (enabled, valid_answer, expected) in [(false, false, 1), (true, true, 1), (true, false, 2)]
    {
        let first = if valid_answer {
            (
                "200 OK",
                json!({"choices":[{"finish_reason":"stop","message":{"content":"错误答案"}}]})
                    .to_string(),
            )
        } else {
            ("500 Internal Server Error", "{}".into())
        };
        let mut steps = vec![(first.0.into(), first.1, Duration::ZERO)];
        if expected == 2 {
            steps.push((
                "503 Service Unavailable".into(),
                "{}".into(),
                Duration::ZERO,
            ));
        }
        let (base, requests) = sequence_fixture(steps);
        let mut selected = profile(base);
        selected.protocol_fallback_enabled = enabled;
        let rule = TestRule {
            prompt: "问题".into(),
            keyword: "正确答案".into(),
            alternative_keywords: Vec::new(),
            timeout_seconds: None,
        };
        let result = api::complete(&selected, &rule, "", 10, &CancellationToken::new()).await;
        assert_eq!(result.attempted_modes.len(), expected);
        if valid_answer {
            assert!(result.error.is_none());
            assert!(!rule.matches(&result.answer));
        } else {
            assert!(result.error.is_some());
        }
        for _ in 0..expected {
            requests.recv_timeout(Duration::from_secs(2)).unwrap();
        }
        assert!(requests.try_recv().is_err());
    }
}

/// 非法或无法精确保存的秒数在网络前失败，不发送请求、不回退，也不能使计时器溢出。
#[tokio::test]
async fn invalid_timeout_fails_before_network_or_protocol_fallback() {
    let mut selected = profile("https://example.com/v1".into());
    selected.protocol_fallback_enabled = true;
    for timeout in [0, 9_007_199_254_740_992, u64::MAX] {
        let result = api::complete(
            &selected,
            &TestRule::default(),
            "",
            timeout,
            &CancellationToken::new(),
        )
        .await;
        assert_eq!(result.error_kind.as_deref(), Some("configuration"));
        assert!(result.attempted_modes.is_empty());
    }
}

/// 首次错误耗时计入同一总预算；回退不能延长到第二个完整期限，预取消不发送请求。
#[tokio::test]
async fn fallback_shares_original_timeout_and_respects_cancellation() {
    let success = json!({"status":"completed","output":[{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"完成"}]}]}).to_string();
    let (base, requests) = sequence_fixture(vec![
        (
            "404 Not Found".into(),
            "{}".into(),
            Duration::from_millis(600),
        ),
        ("200 OK".into(), success, Duration::from_millis(600)),
    ]);
    let mut selected = profile(base);
    selected.protocol_fallback_enabled = true;
    let rule = TestRule {
        prompt: "问题".into(),
        keyword: "完成".into(),
        alternative_keywords: Vec::new(),
        timeout_seconds: None,
    };
    let result = api::complete(&selected, &rule, "", 1, &CancellationToken::new()).await;
    assert_eq!(result.error_kind.as_deref(), Some("timeout"));
    assert_eq!(result.attempted_modes.len(), 2);
    assert_eq!(requests.try_iter().count(), 2);
    let token = CancellationToken::new();
    token.cancel();
    let cancelled = api::complete(&selected, &rule, "", 10, &token).await;
    assert!(cancelled.cancelled);
    assert_eq!(cancelled.attempted_modes.len(), 1);
}

#[test]
fn only_complete_visible_body_can_be_judged() {
    let parsed = api::parse_completion(
        &json!({"choices":[{"finish_reason":"stop","message":{"content":"旧答案", "reasoning_content":"高市早苗"}}]}),
    );
    assert_eq!(parsed.answer, "旧答案");
    assert!(parsed.error.is_none());
    assert!(!parsed.answer.contains("高市早苗"));
    for reason in ["length", "content_filter", "tool_calls", "unknown"] {
        let parsed = api::parse_completion(
            &json!({"choices":[{"finish_reason": reason,"message":{"content":"高市早苗"}}]}),
        );
        assert!(parsed.error.is_some());
    }
    assert!(api::parse_completion(
        &json!({"choices":[{"finish_reason":"stop","message":{"content":"  "}}]})
    )
    .error
    .is_some());
    assert!(
        api::parse_completion(&json!({"choices":[{"message":{"content":"高市早苗"}}]}))
            .error
            .is_some()
    );
    assert!(
        api::parse_completion(&json!({"error":{"message":"高市早苗"}}))
            .error
            .is_some()
    );
}

#[test]
fn responses_parser_reads_only_completed_final_text() {
    let parsed = api::parse_response(&json!({
        "status": "completed",
        "output": [{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"高市早苗"}]}]
    }));
    assert_eq!(parsed.answer, "高市早苗");
    assert!(parsed.error.is_none());
    assert!(api::parse_response(&json!({
        "status": "incomplete",
        "output_text": "高市早苗"
    }))
    .error
    .is_some());
}

/// Markdown显示可以合并视觉文本，但完整原文、历史及关键词匹配不能被去标记或转义改写。
#[test]
fn markdown_answer_roundtrips_without_changing_literal_match() {
    let answer = "**高市**早苗\n\n```javascript\nconst value = '<img src=x>';\n```\n\n[链接](https://example.com)";
    let completion = api::parse_completion(
        &json!({"choices":[{"finish_reason":"stop","message":{"content":answer}}]}),
    );
    assert!(completion.error.is_none());
    assert_eq!(completion.answer, answer);
    assert!(!completion.answer.contains("高市早苗"));
    assert!(completion.answer.contains("**高市**早苗"));
    let mut store = Store::open(std::path::Path::new(":memory:")).unwrap();
    let record = TestRecord {
        id: "markdown".into(),
        profile_id: "one".into(),
        profile_name: "fixture".into(),
        model: "fixture".into(),
        started_at: "2026-10-23T00:00:00Z".into(),
        duration_ms: 10,
        status: "failed".into(),
        answer: completion.answer,
        error: None,
        prompt: "问题".into(),
        keyword: "高市早苗".into(),
        alternative_keywords: Vec::new(),
        rule_name: None,
        rule_version: "literal-contains-v1".into(),
        source: Some("manual".into()),
        interface_mode: Some("chat_completions".into()),
        protocol_fallback_enabled: None,
        attempted_modes: Vec::new(),
        reasoning_effort: Some("low".into()),
        effective_timeout_seconds: Some(10),
        condition_fingerprint: None,
        error_kind: None,
        http_status: None,
        request_id: None,
        answer_type: Some("text".into()),
        answer_parsing_version: Some("html-v1".into()),
    };
    store.save_record(&record).unwrap();
    assert_eq!(store.history().unwrap()[0].answer, answer);
}

#[test]
fn custom_rules_reject_blank_but_preserve_meaningful_spaces() {
    assert!(TestRule {
        prompt: "问题".into(),
        keyword: " \n".into(),
        alternative_keywords: Vec::new(),
        timeout_seconds: None,
    }
    .validate()
    .is_err());
    let rule = TestRule {
        prompt: "自定义问题\n".into(),
        keyword: " Answer ".into(),
        alternative_keywords: Vec::new(),
        timeout_seconds: None,
    };
    assert!(rule.validate().is_ok());
    assert!(!"Answer".contains(&rule.keyword));
    assert!("前缀 Answer 后缀".contains(&rule.keyword));
    assert!(!" answer ".contains(&rule.keyword));
}

#[tokio::test]
async fn real_request_sends_only_prompt_and_uses_complete_response() {
    let body = json!({"choices":[{"finish_reason":"stop","message":{"content":"命中自定义答案"}}]})
        .to_string();
    let (base, receiver) = fixture("200 OK", body, Duration::ZERO);
    let rule = TestRule {
        prompt: "独立题面".into(),
        keyword: "自定义答案".into(),
        alternative_keywords: vec!["do-not-send-alternative-answer".into()],
        timeout_seconds: None,
    };
    let result = api::complete(
        &profile(base),
        &rule,
        "fixture-secret",
        600,
        &CancellationToken::new(),
    )
    .await;
    assert!(result.error.is_none());
    assert_eq!(result.answer, "命中自定义答案");
    let request = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
    let payload: serde_json::Value =
        serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(
        payload["messages"],
        json!([{"role":"user","content":"独立题面"}])
    );
    assert!(payload.get("tools").is_none());
    assert!(!payload.to_string().contains("自定义答案"));
    assert!(!payload
        .to_string()
        .contains("do-not-send-alternative-answer"));
    assert_eq!(payload["stream"], false);
    assert!(request.starts_with("POST /v1/chat/completions "));
}

#[tokio::test]
async fn responses_request_uses_responses_payload_and_parser() {
    let body = json!({
        "status": "completed",
        "output": [{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"命中自定义答案"}]}]
    })
    .to_string();
    let (base, receiver) = fixture("200 OK", body, Duration::ZERO);
    let rule = TestRule {
        prompt: "独立题面".into(),
        keyword: "自定义答案".into(),
        alternative_keywords: Vec::new(),
        timeout_seconds: None,
    };
    let mut selected = profile(base);
    selected.interface_mode = "responses".into();
    selected.reasoning_effort = Some("low".into());
    let result = api::complete(&selected, &rule, "", 10, &CancellationToken::new()).await;
    assert!(result.error.is_none());
    assert_eq!(result.answer, "命中自定义答案");
    let request = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
    let payload: serde_json::Value =
        serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(payload["input"], "独立题面");
    assert_eq!(payload["reasoning"]["effort"], "low");
    assert!(payload.get("messages").is_none());
    assert!(request.starts_with("POST /v1/responses "));
}

#[tokio::test]
async fn errors_do_not_expose_service_response_or_api_key() {
    let (base, _) = fixture(
        "401 Unauthorized",
        "fixture-secret 高市早苗".into(),
        Duration::ZERO,
    );
    let result = api::complete(
        &profile(base),
        &TestRule::default(),
        "fixture-secret",
        10,
        &CancellationToken::new(),
    )
    .await;
    assert!(result.answer.is_empty());
    assert!(result.error.as_ref().unwrap().contains("401"));
    assert!(!result.error.unwrap().contains("fixture-secret"));
}

#[tokio::test]
async fn cancellation_timeout_and_invalid_json_never_judge() {
    let token = CancellationToken::new();
    token.cancel();
    let result = api::complete(
        &profile("http://127.0.0.1:9/v1".into()),
        &TestRule::default(),
        "",
        10,
        &token,
    )
    .await;
    assert!(result.cancelled);
    // 0 秒现在属于配置错误；真实超时需使用合法的 1 秒期限与更慢的本机响应。
    let (base, _) = fixture("200 OK", "{}".into(), Duration::from_millis(1500));
    let result = api::complete(
        &profile(base),
        &TestRule::default(),
        "",
        1,
        &CancellationToken::new(),
    )
    .await;
    assert!(result.error.unwrap().contains("超过"));
    let (base, _) = fixture("200 OK", "非 JSON".into(), Duration::ZERO);
    let result = api::complete(
        &profile(base),
        &TestRule::default(),
        "",
        10,
        &CancellationToken::new(),
    )
    .await;
    assert!(result.error.unwrap().contains("JSON"));
}

#[test]
fn history_keeps_rule_snapshot_and_reopens_without_secrets() {
    let path = std::env::temp_dir().join(format!("model-probe-test-{}.db", Uuid::new_v4()));
    {
        let mut store = Store::open(&path).unwrap();
        let record = TestRecord {
            id: "one".into(),
            profile_id: "profile".into(),
            profile_name: "测试模型".into(),
            model: "fixture-model".into(),
            started_at: "2026-10-23T00:00:00Z".into(),
            duration_ms: 10,
            status: "passed".into(),
            answer: "旧答案".into(),
            error: None,
            prompt: "旧题面".into(),
            keyword: "旧答案".into(),
            alternative_keywords: Vec::new(),
            rule_name: None,
            rule_version: "literal-contains-v1".into(),
            source: Some("manual".into()),
            interface_mode: Some("chat_completions".into()),
            protocol_fallback_enabled: None,
            attempted_modes: Vec::new(),
            reasoning_effort: Some("low".into()),
            effective_timeout_seconds: Some(10),
            condition_fingerprint: None,
            error_kind: None,
            http_status: None,
            request_id: None,
            answer_type: Some("text".into()),
            answer_parsing_version: Some("html-v1".into()),
        };
        store.save_record(&record).unwrap();
        let config = AppConfig {
            rule: TestRule {
                prompt: "新题面".into(),
                keyword: "新答案".into(),
                alternative_keywords: Vec::new(),
                timeout_seconds: None,
            },
            ..AppConfig::default()
        };
        store.save_config(config).unwrap();
    }
    {
        let mut store = Store::open(&path).unwrap();
        assert_eq!(store.config.rule.prompt, "新题面");
        let history = store.history().unwrap();
        assert_eq!(history[0].prompt, "旧题面");
        assert_eq!(history[0].keyword, "旧答案");
        assert!(!serde_json::to_string(&store.config)
            .unwrap()
            .contains("apiKey"));
        store.clear_history().unwrap();
        assert!(store.history().unwrap().is_empty());
    }
    std::fs::remove_file(path).unwrap();
}
