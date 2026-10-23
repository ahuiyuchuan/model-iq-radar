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
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/v1", listener.local_addr().unwrap());
    let (sender, receiver) = mpsc::channel();
    let status = status.to_string();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
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
        let text = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}", response.len());
        let _ = stream.write_all(text.as_bytes());
    });
    (address, receiver)
}

fn profile(base_url: String) -> ModelProfile {
    ModelProfile {
        id: "fixture".into(),
        name: "本地测试".into(),
        base_url,
        model: "fixture-model".into(),
        has_key: false,
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
        rule_version: "literal-contains-v1".into(),
    };
    store.save_record(&record).unwrap();
    assert_eq!(store.history().unwrap()[0].answer, answer);
}

#[test]
fn custom_rules_reject_blank_but_preserve_meaningful_spaces() {
    assert!(TestRule {
        prompt: "问题".into(),
        keyword: " \n".into()
    }
    .validate()
    .is_err());
    let rule = TestRule {
        prompt: "自定义问题\n".into(),
        keyword: " Answer ".into(),
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
    };
    let result = api::complete(
        &profile(base),
        &rule,
        "fixture-secret",
        10,
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
    assert_eq!(payload["stream"], false);
    assert!(request.starts_with("POST /v1/chat/completions "));
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
    let (base, _) = fixture("200 OK", "{}".into(), Duration::from_millis(200));
    let result = api::complete(
        &profile(base),
        &TestRule::default(),
        "",
        0,
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
            rule_version: "literal-contains-v1".into(),
        };
        store.save_record(&record).unwrap();
        let config = AppConfig {
            rule: TestRule {
                prompt: "新题面".into(),
                keyword: "新答案".into(),
            },
            ..AppConfig::default()
        };
        store.save_config(config).unwrap();
    }
    {
        let store = Store::open(&path).unwrap();
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
