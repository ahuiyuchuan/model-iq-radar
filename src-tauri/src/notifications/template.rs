//! 自包含 HTML/纯文本邮件。所有模型/历史字段按不可信原文处理，不渲染 Markdown、远程图片或链接。
//! PNG 由 scripts/generate_mail_assets.mjs 从现有 Lucide 图标生成，运行时通过 CID 内嵌。
//! 模板只读取当次记录，不回查当前配置；完整回答随本次邮件发送，但不进入通知状态摘要。
//! 作者：ahui。

use super::MailEvent;
use crate::model::TestRecord;
use chrono::{DateTime, Local};

const REPOSITORY: &str = "https://github.com/ahuiyuchuan/model-iq-radar";

/// SMTP 使用 multipart/alternative，HTML 无法显示时仍能读取相同字段和完整回答。
pub(super) struct MailContent {
    pub subject: String,
    pub text: String,
    pub html: String,
}

/// 编译期静态资源，没有外部图片请求；实际只附加当前模板引用的图标。
pub(super) const ICONS: &[(&str, &[u8])] = &[
    ("brand", include_bytes!("../../assets/mail/brand.png")),
    ("degraded", include_bytes!("../../assets/mail/degraded.png")),
    (
        "recovered",
        include_bytes!("../../assets/mail/recovered.png"),
    ),
    ("failure", include_bytes!("../../assets/mail/failure.png")),
    ("test", include_bytes!("../../assets/mail/test.png")),
    ("profile", include_bytes!("../../assets/mail/profile.png")),
    ("model", include_bytes!("../../assets/mail/model.png")),
    ("rule", include_bytes!("../../assets/mail/rule.png")),
    ("effort", include_bytes!("../../assets/mail/effort.png")),
    ("protocol", include_bytes!("../../assets/mail/protocol.png")),
    ("source", include_bytes!("../../assets/mail/source.png")),
    ("time", include_bytes!("../../assets/mail/time.png")),
    ("duration", include_bytes!("../../assets/mail/duration.png")),
    (
        "note-degraded",
        include_bytes!("../../assets/mail/note-degraded.png"),
    ),
    (
        "note-recovered",
        include_bytes!("../../assets/mail/note-recovered.png"),
    ),
    (
        "note-failure",
        include_bytes!("../../assets/mail/note-failure.png"),
    ),
    ("github", include_bytes!("../../assets/mail/github.png")),
];

/// 仅插入 HTML 文本节点；保留换行，不允许记录中的标签/属性形成可执行内容或外部资源。
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn icon(name: &str, size: u8) -> String {
    // 邮件客户端可能忽略 aria-label；独立仓库链接同时提供图片替代文本，其余图标纯装饰。
    let alt = if name == "github" { "GitHub" } else { "" };
    format!(
        r##"<img src="cid:radar-{name}@model-iq-radar" width="{size}" height="{size}" alt="{alt}" style="display:block;border:0;outline:none;" />"##
    )
}

/// 固定样式使用表格/内联 CSS，兼容不支持 grid/flex 的邮件客户端；窄屏才改为单列。
fn frame(title: &str, subtitle: &str, symbol: &str, content: &str) -> String {
    let brand = icon("brand", 44);
    let symbol = icon(symbol, 56);
    let github = icon("github", 24);
    format!(
        r##"<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="color-scheme" content="dark">
<title>测智雷达</title>
<style>@media only screen and (max-width:480px){{.mail-outer{{padding:12px!important}}.mail-card{{padding:20px!important}}.metadata-cell{{display:block!important;width:100%!important;box-sizing:border-box!important}}.event-title{{font-size:21px!important}}}}</style>
</head>
<body style="margin:0;padding:0;background-color:#081726;color:#dfebf8;font-family:'Segoe UI','Microsoft YaHei',Arial,sans-serif;">
<table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0" bgcolor="#081726">
<tr>
<td class="mail-outer" align="center" style="padding:24px 16px;">
<table role="presentation" width="620" cellspacing="0" cellpadding="0" border="0" bgcolor="#122538" style="width:100%;max-width:620px;border:1px solid #243d53;border-radius:12px;">
<tr>
<td class="mail-card" style="padding:28px;">
<table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0">
<tr>
<td width="56">{brand}</td>
<td>
<div style="font-size:21px;line-height:28px;font-weight:700;color:#e8f3ff;">测智雷达</div>
<div style="font-size:13px;line-height:20px;color:#8eaac5;">Model IQ Radar</div>
</td>
</tr>
</table>
<div style="height:1px;background-color:#294157;margin:22px 0;">
</div>
<table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0">
<tr>
<td width="74" valign="top">{symbol}</td>
<td valign="top">
<h1 class="event-title" style="margin:0 0 8px;font-size:24px;line-height:32px;color:#e8f3ff;">{title}</h1>
<p style="margin:0;color:#9eb8d1;font-size:14px;line-height:23px;">{subtitle}</p>
</td>
</tr>
</table>
{content}
<div style="height:1px;background-color:#294157;margin:28px 0 18px;">
</div>
<table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0">
<tr>
<td valign="bottom" style="color:#91abc5;font-size:12px;line-height:22px;">结果仅依据自定义答案匹配规则，不代表模型综合能力评估。<br>请返回测智雷达查看完整记录。</td>
<td width="44" align="right" valign="bottom">
<a href="{REPOSITORY}" aria-label="GitHub 项目仓库" title="GitHub 项目仓库" style="display:inline-block;padding:10px 0 0 12px;">{github}</a>
</td>
</tr>
</table>
</td>
</tr>
</table>
</td>
</tr>
</table>
</body>
</html>"##
    )
}

fn metadata_cell(name: &str, label: &str, value: &str) -> String {
    format!(
        r##"<td class="metadata-cell" width="50%" valign="top" style="padding:10px 12px;">
<table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0">
<tr>
<td width="36" valign="top" style="padding-top:2px;">{}</td>
<td valign="top">
<div style="font-size:13px;line-height:21px;color:#93aec9;">{label}</div>
<div style="font-size:13px;line-height:22px;color:#dfebf8;word-break:break-all;overflow-wrap:anywhere;">{}</div>
</td>
</tr>
</table>
</td>"##,
        icon(name, 24),
        escape(value)
    )
}

/// 旧字段缺失不猜测；新记录中的 None 强度表示“不指定”。本机时间显式带 UTC 偏移。
pub(super) fn notification(record: &TestRecord, event: MailEvent) -> MailContent {
    let (title, subtitle, symbol, color, background, note) = match event {
        MailEvent::Degraded => (
            "模型测试结果为降智",
            "本次完整回答未命中任何已配置答案。",
            "degraded",
            "#f5b63f",
            "#2d302d",
            "未命中已配置答案，建议结合下方模型回答复测。",
        ),
        MailEvent::Recovered => (
            "模型状态恢复不降智",
            "本次完整回答已命中已配置答案。",
            "recovered",
            "#50d694",
            "#153b3d",
            "已命中已配置答案，当前结果恢复为不降智。",
        ),
        MailEvent::Failure => (
            "模型测试失败",
            "本次请求未能正常完成，无法进行结果判定。",
            "failure",
            "#f37670",
            "#322936",
            "本次请求未能正常完成。",
        ),
    };
    let time = DateTime::parse_from_rfc3339(&record.started_at)
        .map(|value| {
            let local = value.with_timezone(&Local);
            format!(
                "{} (UTC{})",
                local.format("%Y-%m-%d %H:%M:%S"),
                local.format("%:z")
            )
        })
        .unwrap_or_else(|_| "时间不可用".into());
    let effort = record
        .reasoning_effort
        .as_deref()
        .unwrap_or(if record.interface_mode.is_some() {
            "不指定"
        } else {
            "未记录"
        });
    let protocol = match record
        .attempted_modes
        .last()
        .map(String::as_str)
        .or(record.interface_mode.as_deref())
    {
        Some("responses") => "Responses",
        Some("chat_completions") => "Chat Completions",
        _ => "未记录",
    };
    let source = match record.source.as_deref() {
        Some("manual") => "手动测试",
        Some("automatic") => "自动测试",
        _ => "未记录",
    };
    let rule = record
        .rule_name
        .as_deref()
        .filter(|name| !name.is_empty())
        .unwrap_or("未记录");
    let duration = format!("{:.2} 秒", record.duration_ms as f64 / 1000.0);
    let values = [
        ("profile", "配置名称", record.profile_name.as_str()),
        ("model", "模型", record.model.as_str()),
        ("rule", "测试规则", rule),
        ("effort", "思考等级", effort),
        ("protocol", "实际接口", protocol),
        ("source", "测试来源", source),
        ("time", "测试时间", time.as_str()),
        ("duration", "请求耗时", duration.as_str()),
    ];
    let rows = values
        .chunks_exact(2)
        .map(|pair| {
            format!(
                "<tr>{}{}</tr>",
                metadata_cell(pair[0].0, pair[0].1, pair[0].2),
                metadata_cell(pair[1].0, pair[1].1, pair[1].2)
            )
        })
        .collect::<String>();
    let metadata = format!(
        r##"<table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0" bgcolor="#152b40" style="margin-top:24px;padding:8px;border:1px solid #263f56;border-radius:9px;">{rows}</table>"##
    );
    let failure = matches!(event, MailEvent::Failure);
    let note_title = if failure {
        "错误详情"
    } else {
        "判定说明"
    };
    let error = record
        .error
        .as_deref()
        .filter(|message| !message.is_empty())
        .unwrap_or(note);
    let http = record
        .http_status
        .map(|status| status.to_string())
        .unwrap_or_else(|| "（未获取）".into());
    let request_id = record
        .request_id
        .as_deref()
        .filter(|id| !id.is_empty())
        .unwrap_or("（未获取）");
    let error_details = if failure {
        format!(
            r##"<p style="margin:6px 0 0;color:#9eb8d1;font-size:13px;line-height:22px;">错误类型：{} · 有效超时：{}</p>
<table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0" style="margin-top:12px;">
<tr>
<td class="metadata-cell" width="50%" valign="top" style="font-size:13px;line-height:22px;color:#9eb8d1;">HTTP 状态码<br>
<span style="color:#dfebf8;">{}</span>
</td>
<td class="metadata-cell" width="50%" valign="top" style="font-size:13px;line-height:22px;color:#9eb8d1;">服务商请求编号<br>
<span style="color:#dfebf8;word-break:break-all;">{}</span>
</td>
</tr>
</table>"##,
            escape(record.error_kind.as_deref().unwrap_or("未记录")),
            record
                .effective_timeout_seconds
                .map(|timeout| format!("{timeout} 秒"))
                .unwrap_or_else(|| "未记录".into()),
            escape(&http),
            escape(request_id)
        )
    } else {
        String::new()
    };
    let explanation = if failure { error } else { note };
    let callout = format!(
        r##"<table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0" bgcolor="{background}" style="margin-top:18px;border:1px solid {color};border-radius:9px;">
<tr>
<td style="padding:17px;">
<table role="presentation" cellspacing="0" cellpadding="0" border="0">
<tr>
<td width="32">{}</td>
<td style="color:{color};font-weight:700;font-size:15px;">{note_title}</td>
</tr>
</table>
<p style="margin:14px 0 0;color:#dfebf8;font-size:14px;line-height:24px;white-space:pre-wrap;word-break:break-word;">{}</p>{error_details}</td>
</tr>
</table>"##,
        icon(&format!("note-{symbol}"), 22),
        escape(explanation)
    );
    let answer = if record.answer.is_empty() {
        "本次请求未返回模型回答。"
    } else {
        &record.answer
    };
    let partial_note = if failure && !record.answer.is_empty() {
        "<p style=\"margin:8px 0;color:#9eb8d1;font-size:12px;line-height:21px;\">请求未完成，以下为已收到的回答，不参与结果判定。</p>"
    } else {
        ""
    };
    let response = format!(
        r##"<table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0" bgcolor="#152b40" style="margin-top:18px;border:1px solid #263f56;border-radius:9px;">
<tr>
<td style="padding:17px;">
<h2 style="margin:0 0 12px;font-size:15px;line-height:23px;color:#dfebf8;">模型回答</h2>{partial_note}<pre style="margin:0;font-family:'Segoe UI','Microsoft YaHei',Arial,sans-serif;font-size:14px;line-height:24px;white-space:pre-wrap;word-break:break-all;overflow-wrap:anywhere;color:#dfebf8;">{}</pre>
</td>
</tr>
</table>"##,
        escape(answer)
    );
    let mut text = format!("测智雷达 · {title}\n{subtitle}\n\n");
    for (_, label, value) in values {
        text.push_str(&format!("{label}：{value}\n"));
    }
    text.push_str(&format!("\n{note_title}：{explanation}\n"));
    if failure {
        text.push_str(&format!(
            "错误类型：{}\n有效超时：{}\nHTTP 状态码：{http}\n服务商请求编号：{request_id}\n",
            record.error_kind.as_deref().unwrap_or("未记录"),
            record
                .effective_timeout_seconds
                .map(|value| format!("{value} 秒"))
                .unwrap_or_else(|| "未记录".into())
        ));
    }
    if failure && !record.answer.is_empty() {
        text.push_str("请求未完成，以下为已收到的回答，不参与结果判定。\n");
    }
    text.push_str(&format!("\n模型回答：\n{answer}\n\n结果仅依据自定义答案匹配规则，不代表模型综合能力评估。\n请返回测智雷达查看完整记录。\n{REPOSITORY}"));
    MailContent {
        subject: format!("测智雷达 · {title}"),
        text,
        html: frame(
            title,
            subtitle,
            symbol,
            &format!("{metadata}{callout}{response}"),
        ),
    }
}

/// 主动测试邮件使用同款卡片，但不伪造模型、规则或测智结论。
pub(super) fn test_mail() -> MailContent {
    MailContent {
        subject: "测智雷达 · 测试邮件".into(),
        text: format!("这是一封测智雷达测试邮件。SMTP 发送配置可用。\n软件必须保持运行并联网才能接收后续状态邮件。\n{REPOSITORY}"),
        html: frame("测试邮件已发送", "SMTP 发送配置可用，后续事件邮件将使用相同样式。", "test", "<p style=\"margin:24px 0 0;color:#9eb8d1;font-size:14px;line-height:24px;\">软件必须保持运行并联网才能接收后续状态邮件。此邮件仅检查发送配置，不执行模型测试。</p>"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> TestRecord {
        serde_json::from_value(serde_json::json!({
            "id":"mail-fixture", "profileId":"fixture-profile", "profileName":"主力模型",
            "model":"gpt-6.1", "startedAt":"2026-10-25T10:30:05Z", "durationMs":4640,
            "status":"failed", "answer":"21颗\n这是一条完整的模型回答。", "error":null,
            "prompt":"不应自动附加的题目", "keyword":"不应自动附加的判定答案",
            "ruleVersion":"literal-any-contains-v2", "ruleName":"糖果测试",
            "source":"automatic", "interfaceMode":"chat_completions", "reasoningEffort":"low",
            "effectiveTimeoutSeconds":60, "attemptedModes":["chat_completions","responses"]
        }))
        .unwrap()
    }

    /// 三种事件均使用当次快照与实际最后协议；不把现行配置、题目或匹配答案偷偷带入邮件。
    #[test]
    fn event_templates_use_record_snapshot_and_fixed_repository_link() {
        for (event, title, color) in [
            (MailEvent::Degraded, "模型测试结果为降智", "#f5b63f"),
            (MailEvent::Recovered, "模型状态恢复不降智", "#50d694"),
            (MailEvent::Failure, "模型测试失败", "#f37670"),
        ] {
            let record = sample();
            let content = notification(&record, event);
            for value in [
                title,
                color,
                "主力模型",
                "gpt-6.1",
                "糖果测试",
                "low",
                "Responses",
                "自动测试",
                "4.64 秒",
            ] {
                assert!(content.html.contains(value), "missing {value}");
            }
            assert!(content.text.contains(&record.answer));
            assert!(content.html.contains(&escape(&record.answer)));
            assert!(!content.text.contains(&record.prompt));
            assert!(!content.html.contains(&record.keyword));
            assert!(!content.html.contains("Chat Completions"));
            assert!(content.html.contains("(UTC"));
            assert!(content.html.contains(&format!("href=\"{REPOSITORY}\"")));
            assert!(content.html.contains("alt=\"GitHub\""));
            // 唯一主动链接是右下角仓库图标，不插入远程图片或额外可见链接文案。
            assert_eq!(content.html.matches("<a ").count(), 1);
            assert!(!content.html.contains("src=\"https:"));
        }
    }

    /// 完整回答不截断；任意字段中的标签/属性只能显示为文本，不能执行或请求外部资源。
    #[test]
    fn untrusted_fields_are_escaped_and_long_answers_preserve_the_end() {
        let mut record = sample();
        let unsafe_text =
            "<img src=\"https://untrusted.invalid/track\" onerror='x()'>&<script>x()</script>";
        record.profile_name = unsafe_text.into();
        record.rule_name = Some(unsafe_text.into());
        record.error = Some(unsafe_text.into());
        record.request_id = Some(unsafe_text.into());
        record.answer = format!(
            "{unsafe_text}\n{}\n完整回答结束标记",
            "长回答".repeat(100_000)
        );
        let content = notification(&record, MailEvent::Failure);
        assert!(content.html.contains(&escape(&record.answer)));
        assert!(content.text.contains(&record.answer));
        assert!(content.html.contains("完整回答结束标记"));
        assert!(!content.html.contains(unsafe_text));
        assert!(!content.html.contains("<script>"));
        assert!(!content.html.contains("src=\"https://untrusted"));
        assert!(content.html.contains("以下为已收到的回答，不参与结果判定"));
    }

    /// 旧记录缺字段不猜测低强度/手动来源；失败无回答和部分回答使用明确、不同的空态说明。
    #[test]
    fn missing_fields_and_failure_answers_remain_explicit() {
        let mut record = sample();
        record.interface_mode = None;
        record.attempted_modes.clear();
        record.reasoning_effort = None;
        record.rule_name = None;
        record.source = None;
        record.effective_timeout_seconds = None;
        record.answer.clear();
        let content = notification(&record, MailEvent::Failure);
        assert!(content.text.contains("思考等级：未记录"));
        assert!(content.text.contains("实际接口：未记录"));
        assert!(content.text.contains("测试来源：未记录"));
        assert!(content.text.contains("有效超时：未记录"));
        assert!(content.html.contains("本次请求未返回模型回答。"));
        assert!(!content.html.contains("以下为已收到的回答"));
        record.interface_mode = Some("responses".into());
        assert!(notification(&record, MailEvent::Failure)
            .text
            .contains("思考等级：不指定"));
        record.answer = "部分原文".into();
        record.http_status = Some(429);
        record.request_id = Some("req-fictional".into());
        let content = notification(&record, MailEvent::Failure);
        assert!(content.text.contains("HTTP 状态码：429"));
        assert!(content.text.contains("服务商请求编号：req-fictional"));
        assert!(content.html.contains("以下为已收到的回答，不参与结果判定"));
    }
}
