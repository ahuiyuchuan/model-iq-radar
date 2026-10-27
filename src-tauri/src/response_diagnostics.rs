//! Responses 解析失败的本地取证；只写本次请求已收到的正文与白名单响应元数据。
//! 不持有应用状态锁、不参与测试结算，不发送请求或上传文件。作者：ahui。

use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};
use tokio::sync::Semaphore;
use uuid::Uuid;

/// 与正式请求的既有正文预算一致；只保存收到的前缀，不为取证继续读取超限响应。
pub const BODY_LIMIT_BYTES: usize = 2 * 1024 * 1024;
/// 最多二十组元数据/正文文件；清理只识别本模块生成的文件名，不处理其他用户文件。
const MAX_RECORDS: usize = 20;
/// 文件操作串行，保留策略与新文件创建不能互相竞争；最多四个在途写任务，避免慢盘积压。
static WRITE_LOCK: Mutex<()> = Mutex::new(());
static PENDING_WRITES: Semaphore = Semaphore::const_new(4);

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

/// 仅匹配完整 UTC 时间戳、UUID 和两种扩展名，按文件名顺序保留最多十九组，为新组预留一位。
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
        let Some(suffix) = stem.strip_prefix("responses-") else {
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
}
