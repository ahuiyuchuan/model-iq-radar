//! 正式测试的本地取证：Responses 原始响应与两种协议的独立传输时间线。
//! 不持有应用状态锁、不参与测试结算，不发送请求或上传文件。作者：ahui。

use serde::Serialize;
use std::{
    collections::BTreeMap,
    error::Error,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
use tokio::sync::Semaphore;
use uuid::Uuid;

/// 与正式请求的既有正文预算一致；只保存收到的前缀，不为取证继续读取超限响应。
pub const BODY_LIMIT_BYTES: usize = 2 * 1024 * 1024;
/// 原始响应与 transport 子目录各最多二十组；清理只识别本模块文件，不处理其他用户文件。
const MAX_RECORDS: usize = 20;
/// 文件操作串行，保留策略与新文件创建不能互相竞争；最多四个在途写任务，避免慢盘积压。
static WRITE_LOCK: Mutex<()> = Mutex::new(());
static PENDING_WRITES: Semaphore = Semaphore::const_new(4);

/// 单次协议尝试的纯元数据；与原始响应目录分开限量，不保存题目、回答、地址或鉴权信息。
/// trace_id 仅是本地关联号，不作为请求头发送；attempt_index 从 1 开始，同一次回退共用 trace_id。
#[derive(Default, Serialize)]
pub struct TransportDiagnostic {
    pub format_version: u8,
    pub trace_id: String,
    pub attempt_index: u8,
    pub protocol: String,
    /// 实际请求 JSON 的 stream 值；不能据此断言服务商真的返回了 SSE。
    pub request_stream: bool,
    /// 构建标记区分历史诊断 Beta 与正式双协议流式实现，不作为用户配置或协议判定依据。
    pub build_variant: String,
    pub model: String,
    pub reasoning_effort: Option<String>,
    /// 时间为 UTC；各 after_ms/elapsed_ms 为当前尝试起点的单调时钟耗时，单位毫秒。
    pub started_at_utc: String,
    pub finished_at_utc: String,
    pub elapsed_ms: u64,
    pub total_timeout_seconds: u64,
    /// 回退共享整次测试期限；本次开始的剩余预算不等于重新获得一份总超时。
    pub remaining_budget_at_start_ms: u64,
    /// sending_or_waiting_response_headers 无法进一步证明已发出请求、DNS 成功或 TLS 成功。
    pub stage: String,
    pub send_started_after_ms: Option<u64>,
    pub response_headers_after_ms: Option<u64>,
    pub first_body_after_ms: Option<u64>,
    /// 未收到完整可用响应头时均为 None，不能用 0 或假定 HTTP 200 替代。
    pub http_status: Option<u16>,
    pub http_version: Option<String>,
    pub content_type: Option<String>,
    pub request_id: Option<String>,
    pub received_body_bytes: u64,
    pub body_complete: bool,
    pub body_truncated: bool,
    /// success 只表示请求/协议解析成功，文字是否降智仍由现有业务规则判定。
    pub outcome: String,
    pub error_kind: Option<String>,
    /// application_total_deadline 与 transport 分别表示应用共享期限和网络库报告的超时。
    pub timeout_source: Option<String>,
    pub transport_error: Option<TransportFailure>,
}

impl TransportDiagnostic {
    /// 创建已实际开始的尝试；调用方补入真实 stream 值及构建变体。
    /// 剩余预算由调用方单调时钟计算，不读取用户配置或凭据；格式 2 只新增元数据，旧文件不改写。
    pub fn new(
        trace_id: &str,
        attempt_index: u8,
        protocol: &str,
        model: &str,
        reasoning_effort: Option<&str>,
        total_timeout_seconds: u64,
        remaining_budget_at_start_ms: u64,
    ) -> Self {
        Self {
            format_version: 2,
            trace_id: trace_id.into(),
            attempt_index,
            protocol: protocol.into(),
            model: model.into(),
            reasoning_effort: reasoning_effort.map(str::to_owned),
            started_at_utc: chrono::Utc::now().to_rfc3339(),
            total_timeout_seconds,
            remaining_budget_at_start_ms,
            stage: "initializing_request".into(),
            ..Self::default()
        }
    }
}

/// reqwest 可核实的分类与有界错误链；不根据报错文本猜测中转内部哪一层断开。
#[derive(Serialize)]
pub struct TransportFailure {
    pub is_timeout: bool,
    pub is_connect: bool,
    pub is_request: bool,
    pub is_body: bool,
    pub is_decode: bool,
    pub causes: Vec<TransportCause>,
    pub chain_truncated: bool,
}

/// IO 错误保留系统代码（例如 Windows 连接重置代码）；没有 IO 来源时明确为空。
#[derive(Serialize)]
pub struct TransportCause {
    pub message: String,
    pub io_kind: Option<String>,
    pub raw_os_error: Option<i32>,
}

impl TransportFailure {
    /// 去掉 reqwest 绑定的 URL，再逐层保存最多八层、每层最多 1024 字符的安全错误说明。
    /// 已知请求密钥和疑似鉴权信息直接省略，URL 整体隐藏；从不序列化请求/响应对象。
    pub fn from_error(error: reqwest::Error, key: &str) -> Self {
        let error = error.without_url();
        let mut result = Self {
            is_timeout: error.is_timeout(),
            is_connect: error.is_connect(),
            is_request: error.is_request(),
            is_body: error.is_body(),
            is_decode: error.is_decode(),
            causes: Vec::new(),
            chain_truncated: false,
        };
        let mut cause: Option<&(dyn Error + 'static)> = Some(&error);
        while let Some(current) = cause {
            if result.causes.len() == 8 {
                result.chain_truncated = true;
                break;
            }
            let io_error = current.downcast_ref::<io::Error>();
            result.causes.push(TransportCause {
                message: safe_transport_text(&current.to_string(), key),
                io_kind: io_error.map(|value| format!("{:?}", value.kind())),
                raw_os_error: io_error.and_then(io::Error::raw_os_error),
            });
            cause = current.source();
        }
        result
    }
}

/// 诊断自由文本只来自模型标识或网络库；隐藏 URL/已知密钥及鉴权片段，再限制长度。
/// 无法初始化固定表达式时省略整段文本，不能因诊断失败影响原测试结果。
fn safe_transport_text(value: &str, key: &str) -> String {
    let lower = value.to_ascii_lowercase();
    if (!key.is_empty() && value.contains(key))
        || [
            "authorization",
            "bearer ",
            "basic ",
            "api_key",
            "api-key",
            "password",
            "token=",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
    {
        return "[credential-bearing diagnostic text omitted]".into();
    }
    static URL_PATTERN: OnceLock<Result<regex::Regex, regex::Error>> = OnceLock::new();
    let Ok(pattern) = URL_PATTERN
        .get_or_init(|| regex::Regex::new(r#"(?i)\b(?:https?|socks(?:4a?|5h?)?)://[^\s<>"']+"#))
    else {
        return "[diagnostic text omitted]".into();
    };
    pattern
        .replace_all(value, "[URL omitted]")
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .take(1024)
        .collect()
}

/// 每次正式协议尝试保存一份纯 JSON，包括成功对照；transport 子目录独立保留最多二十份。
/// 共用四个后台许可和文件锁，慢盘、满队列或落盘故障均不阻塞测试、不重试网络请求。
pub fn save_transport_in_background(
    directory: &Path,
    mut diagnostic: TransportDiagnostic,
    key: &str,
) {
    diagnostic.model = safe_transport_text(&diagnostic.model, key);
    diagnostic.reasoning_effort = diagnostic
        .reasoning_effort
        .map(|value| safe_transport_text(&value, key));
    diagnostic.content_type = diagnostic
        .content_type
        .map(|value| safe_transport_text(&value, key));
    diagnostic.request_id = diagnostic
        .request_id
        .map(|value| safe_transport_text(&value, key));
    let Ok(permit) = PENDING_WRITES.try_acquire() else {
        eprintln!("请求传输诊断队列已满；原测试结果保留。");
        return;
    };
    let directory = directory.join("transport");
    drop(tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let result = (|| -> io::Result<()> {
            let metadata = serde_json::to_vec_pretty(&diagnostic).map_err(io::Error::other)?;
            if metadata.len() > 64 * 1024 {
                return Err(io::Error::other(
                    "transport diagnostic exceeds metadata limit",
                ));
            }
            let _guard = WRITE_LOCK
                .lock()
                .map_err(|_| io::Error::other("diagnostic writer unavailable"))?;
            fs::create_dir_all(&directory)?;
            prune(&directory)?;
            let path = directory.join(format!(
                "transport-{}-{}.json",
                chrono::Utc::now().format("%Y%m%dT%H%M%S%.3fZ"),
                Uuid::new_v4()
            ));
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            if let Err(error) = file.write_all(&metadata) {
                drop(file);
                let _ = fs::remove_file(path);
                return Err(error);
            }
            Ok(())
        })();
        if result.is_err() {
            eprintln!("请求传输诊断保存失败；原测试结果保留。");
        }
    }));
}

/// serde_json 报告的行、列及错误原因；scope 指明整段正文或某个 SSE 事件 data。
/// 未读取完整时不尝试解析，因此为 None；事件序号从 1 开始，整段 JSON 没有事件序号。
#[derive(Serialize)]
pub struct JsonParseError {
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub scope: &'static str,
    pub event_index: Option<usize>,
}

/// 本次响应的诊断快照；不包含 URL、请求体、鉴权头、密钥或用户配置。
/// 字节数指 HTTP 客户端交付的正文，不包括 TLS/HTTP 分块边界；完整性只以读取到 EOF 为准。
#[derive(Serialize)]
pub struct ResponseDiagnostic {
    pub format_version: u8,
    pub started_at_utc: String,
    pub finished_at_utc: String,
    pub protocol: &'static str,
    pub http_status: u16,
    pub content_type: Option<String>,
    pub request_id: Option<String>,
    pub error_kind: String,
    pub received_body_bytes: u64,
    pub saved_body_bytes: usize,
    pub body_limit_bytes: usize,
    pub body_complete: bool,
    pub body_truncated: bool,
    pub json_error: Option<JsonParseError>,
    pub body_file: Option<String>,
    /// 已知请求密钥若被上游回显，不改写原文，也不落盘该正文；元数据明确说明省略原因。
    pub body_omission_reason: Option<&'static str>,
    /// 单独以原始字节写文件；不能用有损 UTF-8 转换、格式化或重新序列化替代实际正文。
    #[serde(skip)]
    pub body: Vec<u8>,
}

/// 尽力异步保存；满队列、写入/清理失败仅输出固定本地提示，不覆盖原错误、不阻塞历史结算。
/// 调用前已结束网络读取；不复制密钥到后台任务，正文命中已知密钥时只保存省略标记。
pub fn save_in_background(directory: &Path, mut diagnostic: ResponseDiagnostic, key: &str) {
    if !key.is_empty()
        && diagnostic
            .body
            .windows(key.len())
            .any(|window| window == key.as_bytes())
    {
        diagnostic.body.clear();
        diagnostic.saved_body_bytes = 0;
        diagnostic.body_omission_reason = Some("known_request_credential_in_response");
    }
    let Ok(permit) = PENDING_WRITES.try_acquire() else {
        eprintln!("Responses 响应诊断队列已满；原测试结果保留。");
        return;
    };
    let directory = directory.to_path_buf();
    // 丢弃 JoinHandle 仅分离文件任务；测试不等待磁盘，也不因文件故障重试网络请求。
    drop(tokio::task::spawn_blocking(move || {
        let _permit = permit;
        if save(&directory, diagnostic).is_err() {
            eprintln!("Responses 响应诊断保存失败；原测试结果保留。");
        }
    }));
}

/// 锁内先回收旧组再写新组；UUID 和 create_new 同时防止并发重名或覆盖既有文件。
fn save(directory: &Path, mut diagnostic: ResponseDiagnostic) -> io::Result<()> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("diagnostic writer unavailable"))?;
    fs::create_dir_all(directory)?;
    prune(directory)?;
    let stem = format!(
        "responses-{}-{}",
        chrono::Utc::now().format("%Y%m%dT%H%M%S%.3fZ"),
        Uuid::new_v4()
    );
    let body_name = format!("{stem}.body.bin");
    let body_path = directory.join(&body_name);
    let metadata_path = directory.join(format!("{stem}.json"));
    let mut body_created = false;
    let mut metadata_created = false;
    let result = (|| {
        if diagnostic.body_omission_reason.is_none() {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&body_path)?;
            body_created = true;
            file.write_all(&diagnostic.body)?;
            diagnostic.body_file = Some(body_name);
        }
        let metadata = serde_json::to_vec_pretty(&diagnostic).map_err(io::Error::other)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&metadata_path)?;
        metadata_created = true;
        file.write_all(&metadata)
    })();
    if result.is_err() {
        // 普通写失败清除本次半成品；进程意外退出的残留也按组纳入后续保留上限。
        if body_created {
            let _ = fs::remove_file(&body_path);
        }
        if metadata_created {
            let _ = fs::remove_file(&metadata_path);
        }
    }
    result
}

/// 仅匹配 responses/transport 前缀、完整 UTC 时间戳、UUID 与两种扩展名；每个目录为新组预留一位。
/// 任一清理失败就放弃本次写入，不通过无限追加文件绕过保留上限；不递归删除目录。
fn prune(directory: &Path) -> io::Result<()> {
    let mut records: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some(stem) = name
            .strip_suffix(".body.bin")
            .or_else(|| name.strip_suffix(".json"))
        else {
            continue;
        };
        // 独立目录仍可能由用户放入其他文件；不能只凭前缀及末尾 UUID 删除它们。
        let Some(suffix) = stem
            .strip_prefix("responses-")
            .or_else(|| stem.strip_prefix("transport-"))
        else {
            continue;
        };
        let Some((timestamp, id)) = suffix.split_once('-') else {
            continue;
        };
        if timestamp.len() != 20
            || chrono::NaiveDateTime::parse_from_str(timestamp, "%Y%m%dT%H%M%S%.3fZ").is_err()
            || Uuid::parse_str(id).is_err()
            || id.len() != 36
        {
            continue;
        }
        records
            .entry(stem.to_owned())
            .or_default()
            .push(entry.path());
    }
    let remove_count = records.len().saturating_sub(MAX_RECORDS - 1);
    for (_, paths) in records.into_iter().take(remove_count) {
        for path in paths {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    // 异步用例独占同一全局队列，避免测试自己的 acquire_many 排队影响 try_acquire。
    static BACKGROUND_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    /// 独立随机目录只含合成数据；Drop 清理本测试创建的目录，不触及应用真实诊断。
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("probe-diagnostics-test-{}", Uuid::new_v4()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn metadata(&self) -> Vec<Value> {
            fs::read_dir(&self.0)
                .unwrap()
                .filter_map(|entry| {
                    let path = entry.unwrap().path();
                    (path.extension().is_some_and(|ext| ext == "json"))
                        .then(|| serde_json::from_slice(&fs::read(path).unwrap()).unwrap())
                })
                .collect()
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn diagnostic(body: &[u8]) -> ResponseDiagnostic {
        ResponseDiagnostic {
            format_version: 2,
            started_at_utc: "2026-10-27T00:00:00Z".into(),
            finished_at_utc: "2026-10-27T00:00:01Z".into(),
            protocol: "responses",
            http_status: 200,
            content_type: Some("text/event-stream".into()),
            request_id: Some("fixture-id".into()),
            error_kind: "invalid_json".into(),
            received_body_bytes: body.len() as u64,
            saved_body_bytes: body.len(),
            body_limit_bytes: BODY_LIMIT_BYTES,
            body_complete: true,
            body_truncated: false,
            json_error: Some(JsonParseError {
                message: "expected value".into(),
                line: 1,
                column: 1,
                scope: "response_body",
                event_index: None,
            }),
            body_file: None,
            body_omission_reason: None,
            body: body.to_vec(),
        }
    }

    #[test]
    fn body_is_byte_exact_and_metadata_does_not_embed_it() {
        let directory = Directory::new();
        let body = b": keepalive\r\ndata: \xff\x00\r\n\r\n";
        save(&directory.0, diagnostic(body)).unwrap();
        let metadata = directory.metadata().pop().unwrap();
        assert_eq!(metadata["received_body_bytes"], body.len());
        assert_eq!(metadata["saved_body_bytes"], body.len());
        assert_eq!(metadata["body_complete"], true);
        assert!(metadata.get("body").is_none());
        assert_eq!(
            fs::read(directory.0.join(metadata["body_file"].as_str().unwrap())).unwrap(),
            body
        );
        assert_eq!(metadata["json_error"]["scope"], "response_body");
    }

    #[test]
    fn retention_includes_orphans_and_preserves_unrelated_files() {
        let directory = Directory::new();
        for index in 0..25 {
            let stem = format!("responses-20261027T0000{index:02}.000Z-{}", Uuid::new_v4());
            // 模拟进程退出只写了一半；残留单文件也必须计入二十组预算。
            fs::write(directory.0.join(format!("{stem}.body.bin")), b"old").unwrap();
        }
        let unrelated = format!("responses-my-note-{}.json", Uuid::new_v4());
        fs::write(directory.0.join(&unrelated), b"{}").unwrap();
        fs::create_dir(directory.0.join("user-folder")).unwrap();
        save(&directory.0, diagnostic(b"new")).unwrap();
        assert!(directory.0.join(unrelated).exists());
        assert!(directory.0.join("user-folder").is_dir());
        // 十九个旧单文件、新的一对文件、无关文件与目录。
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 23);
    }

    #[test]
    fn concurrent_saves_have_unique_pairs_and_respect_limit() {
        let directory = Directory::new();
        std::thread::scope(|scope| {
            for _ in 0..24 {
                scope.spawn(|| save(&directory.0, diagnostic(b"synthetic")).unwrap());
            }
        });
        let metadata = directory.metadata();
        assert_eq!(metadata.len(), MAX_RECORDS);
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), MAX_RECORDS * 2);
        for item in metadata {
            assert_eq!(
                fs::read(directory.0.join(item["body_file"].as_str().unwrap())).unwrap(),
                b"synthetic"
            );
        }
    }

    #[test]
    fn file_instead_of_directory_fails_without_overwrite() {
        let directory = Directory::new();
        let path = directory.0.join("not-a-directory");
        fs::write(&path, b"keep").unwrap();
        assert!(save(&path, diagnostic(b"response")).is_err());
        assert_eq!(fs::read(path).unwrap(), b"keep");
    }

    /// 获取全部许可等待后台文件任务完成；不使用不稳定轮询或真实网络/凭据。
    #[tokio::test]
    async fn known_key_is_omitted_without_rewriting_or_copying_credentials() {
        let _test_guard = BACKGROUND_TEST_LOCK.lock().await;
        let directory = Directory::new();
        save_in_background(
            &directory.0,
            diagnostic(b"echo fixture-only-key"),
            "fixture-only-key",
        );
        let _drained = PENDING_WRITES.acquire_many(4).await.unwrap();
        let metadata = directory.metadata().pop().unwrap();
        assert_eq!(
            metadata["body_omission_reason"],
            "known_request_credential_in_response"
        );
        assert_eq!(metadata["saved_body_bytes"], 0);
        assert_eq!(metadata["received_body_bytes"], 21);
        assert_eq!(metadata["body_file"], json!(null));
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
        assert!(!metadata.to_string().contains("fixture-only-key"));
    }

    #[tokio::test]
    async fn failed_background_write_releases_queue_for_following_test() {
        let _test_guard = BACKGROUND_TEST_LOCK.lock().await;
        let directory = Directory::new();
        let path = directory.0.join("occupied");
        fs::write(&path, b"keep").unwrap();
        save_in_background(&path, diagnostic(b"not json"), "");
        let drained = PENDING_WRITES.acquire_many(4).await.unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"keep");
        drop(drained);
        save_in_background(&directory.0, diagnostic(b"next response"), "");
        let _drained = PENDING_WRITES.acquire_many(4).await.unwrap();
        assert_eq!(directory.metadata().len(), 1);
    }

    #[tokio::test]
    async fn full_queue_skips_new_write_without_waiting_or_creating_files() {
        let _test_guard = BACKGROUND_TEST_LOCK.lock().await;
        let _occupied = PENDING_WRITES.acquire_many(4).await.unwrap();
        let directory = Directory::new();
        save_in_background(&directory.0, diagnostic(b"not json"), "");
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
    }

    #[test]
    fn transport_text_omits_credentials_urls_and_bounds_error_messages() {
        assert!(!safe_transport_text("echo fixture-key", "fixture-key").contains("fixture-key"));
        for marker in ["Authorization: secret", "Bearer secret", "password=secret"] {
            assert!(!safe_transport_text(marker, "").contains("secret"));
        }
        let text = safe_transport_text(
            "failed https://fixture.invalid/private?x=secret and socks5://proxy.invalid:1080\nEOF",
            "",
        );
        assert!(!text.contains("fixture.invalid"));
        assert!(!text.contains("proxy.invalid"));
        assert!(!text.contains("secret"));
        assert!(!text.contains('\n'));
        assert!(text.contains("EOF"));
        assert_eq!(
            safe_transport_text(&"字".repeat(2048), "").chars().count(),
            1024
        );
    }

    /// 批量并发写入覆盖唯一命名和实际 transport 留存；不读取用户真实诊断目录。
    #[tokio::test]
    async fn transport_writes_are_unique_bounded_and_do_not_copy_body_or_credentials() {
        let _test_guard = BACKGROUND_TEST_LOCK.lock().await;
        let directory = Directory::new();
        let transport_directory = directory.0.join("transport");
        fs::create_dir(&transport_directory).unwrap();
        fs::write(transport_directory.join("user-note.json"), b"keep").unwrap();
        for _ in 0..6 {
            for index in 1..=4 {
                let mut trace = TransportDiagnostic::new(
                    &Uuid::new_v4().to_string(),
                    index,
                    "responses",
                    "fixture-key",
                    None,
                    600,
                    600_000,
                );
                trace.content_type = Some("text/event-stream; fixture-key".into());
                trace.request_id = Some("fixture-key".into());
                save_transport_in_background(&directory.0, trace, "fixture-key");
            }
            // 当前批次结束后再占下一批许可，避免测试自身触发生产的主动丢弃策略。
            let drained = PENDING_WRITES.acquire_many(4).await.unwrap();
            drop(drained);
        }
        let entries: Vec<_> = fs::read_dir(&transport_directory)
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(entries.len(), MAX_RECORDS + 1);
        for entry in entries {
            if entry.file_name() == "user-note.json" {
                assert_eq!(fs::read(entry.path()).unwrap(), b"keep");
                continue;
            }
            let bytes = fs::read(entry.path()).unwrap();
            assert!(bytes.len() <= 64 * 1024);
            assert!(!String::from_utf8_lossy(&bytes).contains("fixture-key"));
            let metadata: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(metadata["format_version"], 2);
            assert!(metadata.get("body").is_none());
        }
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    }

    /// 满队列与目录被普通文件占用均只丢弃诊断；后续任务仍能落盘且不覆盖已有文件。
    #[tokio::test]
    async fn transport_queue_and_disk_failures_do_not_poison_following_writes() {
        let _test_guard = BACKGROUND_TEST_LOCK.lock().await;
        let directory = Directory::new();
        let trace = || {
            TransportDiagnostic::new(
                "fixture-trace",
                1,
                "chat_completions",
                "fixture-model",
                None,
                300,
                300_000,
            )
        };
        let occupied = PENDING_WRITES.acquire_many(4).await.unwrap();
        save_transport_in_background(&directory.0, trace(), "");
        assert!(!directory.0.join("transport").exists());
        drop(occupied);
        let blocked = directory.0.join("blocked");
        fs::write(&blocked, b"keep").unwrap();
        save_transport_in_background(&blocked, trace(), "");
        let drained = PENDING_WRITES.acquire_many(4).await.unwrap();
        assert_eq!(fs::read(&blocked).unwrap(), b"keep");
        drop(drained);
        save_transport_in_background(&directory.0, trace(), "");
        let _drained = PENDING_WRITES.acquire_many(4).await.unwrap();
        assert_eq!(
            fs::read_dir(directory.0.join("transport")).unwrap().count(),
            1
        );
    }
}
