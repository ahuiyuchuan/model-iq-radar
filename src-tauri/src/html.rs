//! 自动识别回答的 html-v1 语义。只识别格式，不判断动画质量；旧历史不重算结论。
//! 代码块优先，普通文字/行内标签例子不会执行；请求失败的部分 HTML 仍可用于禁止邮件。
//! 作者：ahui。
use regex::Regex;
use std::sync::LazyLock;
/// 持久化时记录识别语义；升级识别规则需显式变更，不能重写旧历史结论。
pub const PARSING_VERSION: &str = "html-v1";
/// 回答格式分类，与模型能力是否降智无关；请求错误/取消另外保持原状态。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnswerKind {
    Text,
    Html,
}

/// 通知状态迁移和发送入口共用的兼容判断。
///
/// 新记录会保存 `answerType`，但 v1.0.2 及更早的历史没有该字段；这些旧记录仍须按
/// 同一份 html-v1 规则识别，避免清空或重建通知状态时把 HTML 误当成文字判定。
pub(crate) fn is_html_answer(answer: &str) -> bool {
    classify(answer) == AnswerKind::Html
}
impl AnswerKind {
    /// 写入历史的稳定类型值，旧字段缺失仍保持未知，不反推过去的判定。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Html => "html",
        }
    }
}
static FENCE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^ {0,3}(?:`{3,}|~{3,})[ \t]*(?:html|htm|svg)[ \t]*\r?$").unwrap()
});
static DOCUMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?im)(?:^|\n)[ \t]*(?:<!doctype\s+html\b|<html\b)").unwrap());
static ROOT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^<(?:!doctype\s+html\b|html|head|body|main|section|article|div|style|script|svg|canvas|table|ul|ol|header|footer|form|p|span|h[1-6]|button|pre|code|blockquote|figure|details|dialog|label|textarea|select|video|audio)(?:[\s>/]|$)").unwrap()
});
static XML_DECLARATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^<\?xml[^>]*>\s*").unwrap());

/// 接受 fenced html/htm/svg、独立完整文档、布局片段及 SVG；没有内容的围栏也属于 HTML，
/// 此时预览失败而不降级为关键词判定。普通文本中的 <img> 和 HTML/svg 单词继续作为文本。
pub fn classify(answer: &str) -> AnswerKind {
    if FENCE.is_match(answer)
        || DOCUMENT.is_match(answer)
        || ROOT.is_match(&XML_DECLARATION.replace(answer.trim(), ""))
    {
        AnswerKind::Html
    } else {
        AnswerKind::Text
    }
}

/// 结算顺序固定为取消、请求错误、HTML、文字判定。HTML 永不进入关键词匹配，
/// 包含判定答案/不完整源码也不能被当作能力恢复；渲染生命周期不影响此函数。
pub fn settle(
    answer: &str,
    rule: &crate::model::TestRule,
    error: bool,
    cancelled: bool,
) -> (AnswerKind, &'static str) {
    let kind = classify(answer);
    let status = if cancelled {
        "cancelled"
    } else if error {
        "error"
    } else if kind == AnswerKind::Html {
        "html"
    } else if rule.matches(answer) {
        "passed"
    } else {
        "failed"
    };
    (kind, status)
}

/// 原生入口的防御校验：截断的 html/svg/布局根和 script/style 不运行；浏览器仍负责一般容错。
/// 预览上限由文档仓库处理、取消由前端移除 iframe，不能因为失败重新执行关键词判定。
pub fn preview_issue(source: &str) -> Option<&'static str> {
    static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<[a-z][^>]*>").unwrap());
    static ROOT_TAG: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^\s*<(?:!doctype\s+html[^>]*>\s*<)?([a-z][a-z0-9]*)\b").unwrap()
    });
    if source.trim().is_empty() || !TAG.is_match(source) {
        return Some("没有可运行的 HTML。");
    }
    let root = ROOT_TAG
        .captures(source)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_ascii_lowercase());
    let mut tags = vec!["script".to_owned(), "style".to_owned()];
    if let Some(root) = root.filter(|r| {
        [
            "html", "svg", "div", "main", "section", "article", "canvas", "table", "ul", "ol",
            "form",
        ]
        .contains(&r.as_str())
    }) {
        tags.push(root);
    }
    for tag in tags {
        let open = Regex::new(&format!(r"(?i)<{tag}\b[^>]*>")).unwrap();
        let close = Regex::new(&format!(r"(?i)</{tag}\s*>")).unwrap();
        if open
            .find_iter(source)
            .filter(|m| !m.as_str().ends_with("/>"))
            .count()
            > close.find_iter(source).count()
        {
            return Some("HTML 文档或标签未闭合。");
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 前后端读取同一份样例，验证格式识别、代码围栏优先及普通示例边界。
    #[test]
    fn shared_classification_cases() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/html-answers.json")).unwrap();
        for case in cases.as_array().unwrap() {
            assert_eq!(
                classify(case["answer"].as_str().unwrap()).as_str(),
                case["kind"].as_str().unwrap(),
                "{}",
                case["name"]
            );
        }
    }
    #[test]
    fn html_never_keyword_judged_and_request_states_take_precedence() {
        let rule = crate::model::TestRule {
            keyword: "svg".into(),
            ..Default::default()
        };
        let answer = "<svg><circle r=10 /></svg>";
        assert_eq!(
            settle(answer, &rule, false, false),
            (AnswerKind::Html, "html")
        );
        assert_eq!(
            settle(answer, &rule, true, false),
            (AnswerKind::Html, "error")
        );
        assert_eq!(
            settle(answer, &rule, true, true),
            (AnswerKind::Html, "cancelled")
        );
        assert_eq!(
            settle("svg", &rule, false, false),
            (AnswerKind::Text, "passed")
        );
        assert_eq!(
            settle("missing", &rule, false, false),
            (AnswerKind::Text, "failed")
        );
    }
    #[test]
    fn refuses_truncated_documents_but_allows_svg_and_fragments() {
        for source in [
            "<html><body>",
            "<svg><circle/>",
            "<div>unfinished",
            "<script>while(true){}",
        ] {
            assert!(preview_issue(source).is_some());
        }
        for source in [
            "<div>ok</div>",
            "<svg><circle r=10 /></svg>",
            "<!doctype html><html><body>ok</body></html>",
        ] {
            assert!(preview_issue(source).is_none());
        }
    }
}
