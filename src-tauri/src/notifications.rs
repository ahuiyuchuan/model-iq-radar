//! 邮件通知与状态去重。邮件不参与模型判定，不自动重试；正文包含当次完整回答，
//! 去重状态只保存条件摘要，不复制题目/回答。HTML 转义原文，图标通过 CID 本地内嵌。
//! SMTP 授权码仅存 Windows 凭据管理器；调用方须在释放应用状态锁后等待网络。
//! 作者：ahui。

use crate::model::TestRecord;
use keyring::Entry;
use lettre::{
    message::{header::ContentType, Attachment, Mailbox, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials,
    Address, AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};

mod template;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

// SMTP 与模型 API 使用不同服务名，导入的 profile ID 不能碰撞邮件凭据账户。
const CREDENTIAL_SERVICE: &str = "com.ahui.model-probe.smtp";
const CREDENTIAL_ACCOUNT: &str = "smtp-authorization";
const MAX_TRACKED_CONDITIONS: usize = 1000;
static MAIL_BUSY: AtomicBool = AtomicBool::new(false);

/// SMTP 配置只有非敏感字段。缺省关闭；仅 TLS / STARTTLS，禁止明文登录。
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EmailNotificationSettings {
    pub enabled: bool,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub security: String,
    pub sender: String,
    pub username: String,
    pub recipients: Vec<String>,
    pub on_degraded: bool,
    pub on_recovered: bool,
    pub on_failure: bool,
    /// 仅指示凭据存在；备份必须清除此标志，不能把它当作密码本身。
    pub has_password: bool,
}

impl Default for EmailNotificationSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            smtp_host: String::new(),
            smtp_port: 465,
            security: "tls".into(),
            sender: String::new(),
            username: String::new(),
            recipients: Vec::new(),
            on_degraded: true,
            on_recovered: true,
            on_failure: true,
            has_password: false,
        }
    }
}

/// 表单草稿：密码仅在调用期间存在；空字符串表示保留，removePassword 表示明确删除。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailSettingsInput {
    pub settings: EmailNotificationSettings,
    pub password: String,
    pub remove_password: bool,
}

/// 验证 SMTP 非敏感配置。关闭通知时允许空白初始表单；测试邮件仍要求完整配置。
pub fn validate_settings(settings: &EmailNotificationSettings) -> Result<(), String> {
    if !matches!(settings.security.as_str(), "tls" | "starttls") {
        return Err("SMTP 安全模式只能选择 TLS 或 STARTTLS。".into());
    }
    if settings.smtp_port == 0
        || settings.smtp_host.len() > 253
        || settings.username.len() > 254
        || settings.username.contains(['\r', '\n'])
        || settings.recipients.len() > 10
    {
        return Err("SMTP 端口应为 1—65535，收件邮箱最多 10 个，账号不能包含换行。".into());
    }
    if settings.smtp_host.is_empty()
        && !settings.enabled
        && settings.sender.is_empty()
        && settings.username.is_empty()
        && settings.recipients.is_empty()
    {
        return Ok(());
    }
    // Host::parse 可验证国际域名/IP，但拒绝 URL 协议、路径、查询及用户名。
    if settings.smtp_host.trim() != settings.smtp_host
        || settings
            .smtp_host
            .contains(['/', '\\', '@', '?', '#', '\r', '\n'])
        || url::Host::parse(&settings.smtp_host).is_err()
    {
        return Err("SMTP 服务器请只填写域名或 IP，不要包含协议、路径或账户。".into());
    }
    if settings.sender.parse::<Address>().is_err()
        || settings.sender.len() > 254
        || settings.recipients.is_empty()
        || settings
            .recipients
            .iter()
            .any(|address| address.len() > 254 || address.parse::<Address>().is_err())
    {
        return Err("请填写有效的发件邮箱及 1—10 个收件邮箱。".into());
    }
    Ok(())
}

fn credential() -> Result<Entry, String> {
    Entry::new(CREDENTIAL_SERVICE, CREDENTIAL_ACCOUNT)
        .map_err(|_| "无法访问 Windows 凭据管理器。".into())
}

/// 区分尚未配置与读取失败；不能静默用空密码替换已有凭据。
pub fn read_password() -> Result<Option<String>, String> {
    match credential()?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err("无法读取 SMTP 授权码，请检查 Windows 凭据管理器。".into()),
    }
}

/// 只更新系统凭据；数据库事务失败时调用方应以此前读取的值回滚。
pub fn save_password(value: Option<&str>) -> Result<(), String> {
    let entry = credential()?;
    match value {
        Some(password) => entry
            .set_password(password)
            .map_err(|_| "SMTP 授权码保存失败。".into()),
        None => match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err("SMTP 授权码移除失败。".into()),
        },
    }
}

/// 确认草稿将使用的密码，但不写数据库/凭据。更换鉴权目标须明确输入新密码，
/// 防止测试草稿把旧服务商授权码发送到任意新地址；同一目标可安全保留旧密码。
pub fn validate_input(
    previous: &EmailNotificationSettings,
    input: &EmailSettingsInput,
) -> Result<Option<String>, String> {
    validate_settings(&input.settings)?;
    if input.password.len() > 2500
        || input.password.contains(['\r', '\n'])
        || (input.remove_password && !input.password.is_empty())
    {
        return Err("SMTP 授权码过长、包含换行，或同时选择了替换与移除。".into());
    }
    if input.remove_password {
        if input.settings.enabled {
            return Err("移除授权码前请先关闭邮箱通知。".into());
        }
        return Ok(None);
    }
    if !input.password.is_empty() {
        return Ok(Some(input.password.clone()));
    }
    if previous.has_password
        && (previous.smtp_host != input.settings.smtp_host
            || previous.smtp_port != input.settings.smtp_port
            || previous.security != input.settings.security
            || previous.username != input.settings.username
            || previous.sender != input.settings.sender)
    {
        return Err("SMTP 连接或账号已修改，请明确输入用于新配置的授权码。".into());
    }
    // 导入备份会清除 hasPassword。即使系统中残留旧凭据，也不能将其隐式交给导入目标。
    let password = if previous.has_password {
        read_password()?
    } else {
        None
    };
    if input.settings.enabled && password.is_none() {
        return Err("启用邮箱通知前请填写 SMTP 授权码。".into());
    }
    Ok(password)
}

/// 事件本身不复制回答；发送时读取当次记录。恢复只指同一条件先降智，再得到不降智。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MailEvent {
    Degraded,
    Recovered,
    Failure,
}

impl MailEvent {
    pub fn enabled(self, settings: &EmailNotificationSettings) -> bool {
        settings.enabled
            && match self {
                Self::Degraded => settings.on_degraded,
                Self::Recovered => settings.on_recovered,
                Self::Failure => settings.on_failure,
            }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
struct ConditionState {
    degraded: bool,
    last_failure: Option<String>,
}

/// 按 profile + 已哈希的完整请求条件跟踪，最多 1000 组；不存题目和密钥。
/// 状态在尝试发送前推进，发送失败不排队补发；重启可从历史重建，防止重复邮件。
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct NotificationTracker {
    states: HashMap<String, ConditionState>,
    insertion_order: VecDeque<String>,
}

impl NotificationTracker {
    /// 恢复独立持久化状态并校验资源界限；只允许有界去重摘要，不接受任意原文。
    /// 队列依据实际状态重建，避免旧版本或损坏数据导致淘汰顺序不完整。
    pub fn restore(payload: &str) -> Result<Self, String> {
        if payload.len() > 2 * 1024 * 1024 {
            return Err("邮件通知状态超过安全大小，请先备份本地数据库。".into());
        }
        let mut tracker: Self = serde_json::from_str(payload)
            .map_err(|_| "邮件通知状态格式无效，请先备份本地数据库。")?;
        if tracker.states.len() > MAX_TRACKED_CONDITIONS
            || tracker.insertion_order.len() > MAX_TRACKED_CONDITIONS
            || tracker.states.iter().any(|(key, state)| {
                key.is_empty()
                    || key.len() > 256
                    || key.chars().any(char::is_control)
                    || state.last_failure.as_ref().is_some_and(|failure| {
                        failure.len() > 256 || failure.chars().any(char::is_control)
                    })
            })
        {
            return Err("邮件通知状态超出安全范围，请先备份本地数据库。".into());
        }
        let mut order = VecDeque::new();
        for key in tracker.insertion_order {
            if tracker.states.contains_key(&key) && !order.contains(&key) {
                order.push_back(key);
            }
        }
        for key in tracker.states.keys() {
            if !order.contains(key) {
                order.push_back(key.clone());
            }
        }
        tracker.insertion_order = order;
        Ok(tracker)
    }

    /// history 由存储按最新在前返回，因此倒序重放；旧记录缺条件快照时跳过。
    pub fn from_history(history: &[TestRecord]) -> Self {
        let mut tracker = Self::default();
        for record in history.iter().rev() {
            let _ = tracker.consider(record);
        }
        tracker
    }

    /// 取消不更新状态、不触发故障；相同错误类型+HTTP 状态连续发生只发送一次。
    pub fn consider(&mut self, record: &TestRecord) -> Option<MailEvent> {
        if record.status == "cancelled" {
            return None;
        }
        let fingerprint = record.condition_fingerprint.as_ref()?;
        let key = format!("{}:{fingerprint}", record.profile_id);
        if key.len() > 256 || key.chars().any(char::is_control) {
            return None;
        }
        if !self.states.contains_key(&key) {
            if self.states.len() >= MAX_TRACKED_CONDITIONS {
                if let Some(oldest) = self.insertion_order.pop_front() {
                    self.states.remove(&oldest);
                }
            }
            self.insertion_order.push_back(key.clone());
        }
        let state = self.states.entry(key).or_default();
        match record.status.as_str() {
            "failed" => {
                let notify = !state.degraded;
                state.degraded = true;
                state.last_failure = None;
                notify.then_some(MailEvent::Degraded)
            }
            "passed" => {
                let notify = state.degraded;
                state.degraded = false;
                state.last_failure = None;
                notify.then_some(MailEvent::Recovered)
            }
            "error" => {
                // 旧数据库内容同样不可信，不把任意 errorKind 原文写入去重摘要。
                let kind = record
                    .error_kind
                    .as_deref()
                    .filter(|kind| {
                        matches!(
                            *kind,
                            "protocol"
                                | "tool_call"
                                | "incomplete"
                                | "filtered"
                                | "empty_response"
                                | "configuration"
                                | "connection"
                                | "timeout"
                                | "authentication"
                                | "rate_limit"
                                | "redirect"
                                | "request_rejected"
                                | "http"
                                | "response_too_large"
                                | "read_interrupted"
                                | "invalid_json"
                                | "credentials"
                                | "client"
                        )
                    })
                    .unwrap_or("unknown");
                let failure = format!("{}:{}", kind, record.http_status.unwrap_or(0));
                let notify = state.last_failure.as_ref() != Some(&failure);
                state.last_failure = Some(failure);
                notify.then_some(MailEvent::Failure)
            }
            _ => None,
        }
    }
}

/// 同时最多一封网络发送；不积压重试/补发队列，测试邮件与通知共用发送门禁。
struct MailGuard;
impl MailGuard {
    fn acquire() -> Result<Self, String> {
        MAIL_BUSY
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Self)
            .map_err(|_| "邮件正在发送，请稍后重试。".into())
    }
}
impl Drop for MailGuard {
    fn drop(&mut self) {
        MAIL_BUSY.store(false, Ordering::Release);
    }
}

/// 纯文本与 HTML 为等价正文；HTML 引用的固定 PNG 放在 related 部分，离线也能显示。
/// 不把回答当作附件/HTML，不依赖外部图片，不包含凭据；收件人仍由校验后的配置指定。
fn build_message(
    settings: &EmailNotificationSettings,
    content: template::MailContent,
) -> Result<Message, String> {
    let sender: Mailbox = settings.sender.parse().map_err(|_| "发件邮箱格式无效。")?;
    let mut builder = Message::builder().from(sender).subject(content.subject);
    for address in &settings.recipients {
        builder = builder.to(address.parse().map_err(|_| "收件邮箱格式无效。")?);
    }
    let icons: Vec<_> = template::ICONS
        .iter()
        .filter(|(name, _)| {
            content
                .html
                .contains(&format!("src=\"cid:radar-{name}@model-iq-radar\""))
        })
        .collect();
    let mut html = MultiPart::related().singlepart(SinglePart::html(content.html));
    for (name, bytes) in icons {
        let cid = format!("radar-{name}@model-iq-radar");
        html = html.singlepart(Attachment::new_inline(cid).body(
            bytes.to_vec(),
            ContentType::parse("image/png").map_err(|_| "邮件图标格式无效。")?,
        ));
    }
    builder
        .multipart(
            MultiPart::alternative()
                .singlepart(SinglePart::plain(content.text))
                .multipart(html),
        )
        .map_err(|_| "无法生成邮件，请检查邮箱配置。".into())
}

async fn send(
    settings: &EmailNotificationSettings,
    password: &str,
    content: template::MailContent,
) -> Result<(), String> {
    let _guard = MailGuard::acquire()?;
    let mut complete = settings.clone();
    complete.enabled = true;
    validate_settings(&complete)?;
    if password.is_empty() {
        return Err("请先填写 SMTP 授权码。".into());
    }
    let message = build_message(settings, content)?;
    let builder = if settings.security == "tls" {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&settings.smtp_host)
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&settings.smtp_host)
    }
    .map_err(|_| "无法初始化加密 SMTP 连接。")?;
    let username = if settings.username.is_empty() {
        &settings.sender
    } else {
        &settings.username
    };
    let transport = builder
        .port(settings.smtp_port)
        .timeout(Some(Duration::from_secs(15)))
        .credentials(Credentials::new(username.clone(), password.into()))
        .build();
    // 外层期限同时限制 DNS、握手、鉴权和发送；错误不回显底层 SMTP 文本或凭据。
    match tokio::time::timeout(Duration::from_secs(20), transport.send(message)).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(_)) => {
            Err("邮件发送失败，请检查 SMTP、TLS、账户和授权码；测智结果不受影响。".into())
        }
        Err(_) => Err("邮件发送超过 20 秒，已停止等待；不会自动重试。".into()),
    }
}

/// 发送用户主动发起的测试邮件；使用当前草稿，不保存配置或改变测试结果。
pub async fn send_test(settings: &EmailNotificationSettings, password: &str) -> Result<(), String> {
    send(settings, password, template::test_mail()).await
}

/// 独立发送当次状态与完整回答；调用方处理错误但不得影响已保存结果，也不得自动补发。
pub async fn send_notification(
    settings: &EmailNotificationSettings,
    password: &str,
    record: &TestRecord,
    event: MailEvent,
) -> Result<(), String> {
    if !event.enabled(settings) || record.status == "cancelled" {
        return Ok(());
    }
    send(settings, password, template::notification(record, event)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(status: &str) -> TestRecord {
        serde_json::from_value(serde_json::json!({
            "id":"test", "profileId":"profile", "profileName":"测试配置", "model":"fixture",
            "startedAt":"2026-10-25T00:00:00Z", "durationMs":10, "status":status,
            "answer":"模型完整回答", "error":null, "prompt":"不应自动附加的题目",
            "keyword":"答案", "ruleVersion":"literal-contains-v1", "conditionFingerprint":"scope-a",
            "errorKind":"timeout", "httpStatus":null
        }))
        .unwrap()
    }

    /// 状态按配置和完整条件隔离；取消与重复异常不产生邮件，重启重放也不补发。
    #[test]
    fn deduplicates_conditions_cancellation_and_rebuilds_from_history() {
        let mut tracker = NotificationTracker::default();
        assert_eq!(tracker.consider(&record("passed")), None);
        assert_eq!(
            tracker.consider(&record("failed")),
            Some(MailEvent::Degraded)
        );
        assert_eq!(tracker.consider(&record("failed")), None);
        assert_eq!(tracker.consider(&record("error")), Some(MailEvent::Failure));
        assert_eq!(tracker.consider(&record("cancelled")), None);
        assert_eq!(tracker.consider(&record("error")), None);
        assert_eq!(
            tracker.consider(&record("passed")),
            Some(MailEvent::Recovered)
        );
        assert_eq!(tracker.consider(&record("passed")), None);
        let mut other = record("failed");
        other.condition_fingerprint = Some("scope-b".into());
        assert_eq!(tracker.consider(&other), Some(MailEvent::Degraded));
        let mut restored = NotificationTracker::from_history(&[record("failed"), record("passed")]);
        assert_eq!(restored.consider(&record("failed")), None);
        assert_eq!(
            restored.consider(&record("passed")),
            Some(MailEvent::Recovered)
        );
    }

    /// 独立通知状态不依赖可清理的历史；无界条件输入被淘汰，恢复仍遵守同一上限。
    #[test]
    fn persisted_tracker_survives_history_cleanup_and_stays_bounded() {
        let mut tracker = NotificationTracker::default();
        let _ = tracker.consider(&record("failed"));
        let _ = tracker.consider(&record("error"));
        let payload = serde_json::to_string(&tracker).unwrap();
        let mut restored = NotificationTracker::restore(&payload).unwrap();
        assert_eq!(restored.consider(&record("error")), None);
        assert_eq!(
            restored.consider(&record("passed")),
            Some(MailEvent::Recovered)
        );
        for index in 0..=MAX_TRACKED_CONDITIONS {
            let mut row = record("failed");
            row.condition_fingerprint = Some(format!("condition-{index}"));
            let _ = restored.consider(&row);
        }
        assert_eq!(restored.states.len(), MAX_TRACKED_CONDITIONS);
        assert_eq!(restored.insertion_order.len(), MAX_TRACKED_CONDITIONS);
        assert!(NotificationTracker::restore(&serde_json::to_string(&restored).unwrap()).is_ok());
        assert!(NotificationTracker::restore("{invalid-json").is_err());
    }

    /// 默认关闭无需提供邮箱；真正启用时不能接受 URL、明文模式或多行账号。
    #[test]
    fn validates_transport_and_mailbox_boundaries() {
        assert!(validate_settings(&EmailNotificationSettings::default()).is_ok());
        let mut settings = EmailNotificationSettings {
            enabled: true,
            smtp_host: "smtp.example.com".into(),
            sender: "sender@example.com".into(),
            recipients: vec!["reader@example.com".into()],
            ..Default::default()
        };
        assert!(validate_settings(&settings).is_ok());
        settings.smtp_host = "https://smtp.example.com/path".into();
        assert!(validate_settings(&settings).is_err());
        settings.smtp_host = "smtp.example.com".into();
        settings.security = "plain".into();
        assert!(validate_settings(&settings).is_err());
        settings.security = "starttls".into();
        settings.username = "sender\r\nBcc: attacker@example.com".into();
        assert!(validate_settings(&settings).is_err());
    }

    /// 不进行真实外发：非敏感设置没有密码字段，实际邮件包含文本/HTML 与本地 CID 图标。
    #[test]
    fn serialization_excludes_authorization_code() {
        let value = serde_json::to_value(EmailNotificationSettings::default()).unwrap();
        assert!(value.get("password").is_none());
        assert!(value.get("authorizationCode").is_none());
        let settings = EmailNotificationSettings {
            sender: "sender@example.com".into(),
            recipients: vec!["reader@example.com".into()],
            ..Default::default()
        };
        let message = build_message(&settings, template::test_mail()).unwrap();
        let mime = String::from_utf8(message.formatted()).unwrap();
        assert!(mime.contains("multipart/alternative"));
        assert!(mime.contains("multipart/related"));
        assert!(mime.contains("text/plain"));
        assert!(mime.contains("text/html"));
        assert!(mime.contains("Content-ID: <radar-github@model-iq-radar>"));
        assert!(mime.contains("Content-Disposition: inline"));
        assert!(!mime.contains("radar-degraded@model-iq-radar"));
    }

    /// 只构造虚构记录，不发送邮件。显式指定 QA 目录时导出真实模板和 MIME，供本地视觉验收。
    #[test]
    fn notification_mime_embeds_only_referenced_icons() {
        let settings = EmailNotificationSettings {
            sender: "sender@example.com".into(),
            recipients: vec!["reader@example.com".into()],
            ..Default::default()
        };
        for (name, event, status) in [
            ("degraded", MailEvent::Degraded, "failed"),
            ("recovered", MailEvent::Recovered, "passed"),
            ("failure", MailEvent::Failure, "error"),
            ("long", MailEvent::Failure, "error"),
        ] {
            let mut record = record(status);
            record.profile_name = "主力模型".into();
            record.model = "gpt-6.1".into();
            record.rule_name = Some("糖果测试".into());
            record.started_at = "2026-10-25T10:30:05Z".into();
            record.duration_ms = 4640;
            record.reasoning_effort = Some("low".into());
            record.source = Some("automatic".into());
            record.interface_mode = Some("chat_completions".into());
            record.attempted_modes = vec!["chat_completions".into(), "responses".into()];
            record.effective_timeout_seconds = Some(60);
            record.keyword = "21颗".into();
            record.answer = match name {
                "failure" => String::new(),
                "degraded" => "20颗\n这是当次模型返回的完整原文。".into(),
                _ => "21颗\n这是当次模型返回的完整原文。".into(),
            };
            record.error =
                (name == "failure").then(|| "请求超时：在有效超时 60 秒内未获得完整响应。".into());
            if name == "long" {
                record.profile_name = "长配置名称与英文连续标识".repeat(10);
                record.model = "long-model-identifier".repeat(20);
                record.rule_name = Some("长规则名称".repeat(12));
                record.request_id = Some("req-fictional-".repeat(20));
                record.error = Some("响应读取中断，以下为已收到的部分原文。".into());
                record.answer = format!(
                    "<img src=\"https://untrusted.invalid/track\">\n{}\n完整回答结束标记",
                    "连续长原文abc123".repeat(200)
                );
            }
            let content = template::notification(&record, event);
            let html = content.html.clone();
            let message = build_message(&settings, content).unwrap();
            let bytes = message.formatted();
            let mime = String::from_utf8_lossy(&bytes);
            for (icon, png) in template::ICONS {
                assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
                assert_eq!(
                    mime.contains(&format!("Content-ID: <radar-{icon}@model-iq-radar>")),
                    html.contains(&format!("src=\"cid:radar-{icon}@model-iq-radar\"")),
                    "{name}: {icon}"
                );
            }
            if let Some(directory) = std::env::var_os("MODEL_PROBE_EMAIL_PREVIEW_DIR") {
                let directory = std::path::PathBuf::from(directory);
                std::fs::create_dir_all(&directory).unwrap();
                std::fs::write(directory.join(format!("{name}.html")), html).unwrap();
                std::fs::write(directory.join(format!("{name}.eml")), bytes).unwrap();
            }
        }
    }

    /// 即使调用方误传事件，手动取消也必须在读取 SMTP/网络前直接结束。
    #[tokio::test]
    async fn cancelled_notification_never_uses_smtp() {
        let settings = EmailNotificationSettings {
            enabled: true,
            ..Default::default()
        };
        assert!(
            send_notification(&settings, "", &record("cancelled"), MailEvent::Failure)
                .await
                .is_ok()
        );
    }

    /// 本机 SMTP 故意不提供 STARTTLS；必须拒绝降级，不能发送账号/授权码或泄露服务文本。
    /// 只走随机本机端口，不发送真实邮件，也不使用 Windows 中保存的任何凭据。
    #[tokio::test]
    async fn smtp_requires_encryption_before_authentication() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            stream.write_all(b"220 fixture.local ESMTP\r\n").unwrap();
            let mut bytes = [0; 4096];
            let count = stream.read(&mut bytes).unwrap();
            let request = String::from_utf8_lossy(&bytes[..count]).to_string();
            stream
                .write_all(b"250-fixture.local\r\n250 AUTH PLAIN LOGIN\r\n")
                .unwrap();
            let mut transcript = request;
            if let Ok(count) = stream.read(&mut bytes) {
                transcript.push_str(&String::from_utf8_lossy(&bytes[..count]));
            }
            transcript
        });
        let settings = EmailNotificationSettings {
            smtp_host: "127.0.0.1".into(),
            smtp_port: port,
            security: "starttls".into(),
            sender: "sender@example.com".into(),
            recipients: vec!["reader@example.com".into()],
            ..Default::default()
        };
        let error = send_test(&settings, "fixture-smtp-secret")
            .await
            .unwrap_err();
        assert!(error.contains("邮件发送失败"));
        assert!(!error.contains("fixture-smtp-secret"));
        let transcript = worker.join().unwrap();
        assert!(transcript.starts_with("EHLO "));
        assert!(!transcript.contains("AUTH "));
        assert!(!transcript.contains("sender@example.com"));
        assert!(!transcript.contains("fixture-smtp-secret"));
    }
}
