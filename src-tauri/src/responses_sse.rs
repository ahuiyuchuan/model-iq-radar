//! Responses SSE 的有界正文解析；网络读取、超时和取消仍由 api.rs 统一管理。
//! 只提取明确结束事件内的完整 response，不用 delta 拼答案，不执行正文中的任何指令。作者：ahui。

use serde_json::Value;

/// 固定用户提示与可选 JSON 位置；错误中不回显事件原文、服务端 message 或敏感元数据。
#[derive(Debug)]
pub struct StreamFailure {
    pub kind: &'static str,
    pub message: &'static str,
    pub json_error: Option<serde_json::Error>,
    /// 从 1 开始的数据事件序号；0 表示整段 UTF-8 或缺少结束事件等流级错误。
    pub event_index: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 使用合成正文覆盖协议边界，不复制用户取证日志或服务商元数据。
    fn completed() -> String {
        format!(
            "event: response.completed\ndata: {}\n\n",
            json!({
                "type":"response.completed", "response":{
                    "id":"resp_fixture", "status":"completed", "output":[
                        {"type":"reasoning", "summary":[]},
                        {"type":"message", "role":"assistant", "status":"completed", "phase":"commentary", "content":[{"type":"output_text", "text":"中间说明"}]},
                        {"type":"message", "role":"assistant", "status":"completed", "phase":"final_answer", "content":[{"type":"output_text", "text":"21颗"}]}
                    ]
                }
            })
        )
    }

    #[test]
    fn final_response_keeps_existing_final_text_validation() {
        let stream = format!(": keepalive\n\ndata: {{\"type\":\"response.output_text.delta\",\"delta\":\"错误增量\"}}\n\n{}", completed());
        let result = crate::api::parse_response(&parse(stream.as_bytes()).unwrap());
        assert_eq!(result.answer, "21颗");
        assert!(result.error.is_none());
    }

    #[test]
    fn supports_bom_line_endings_multiline_data_and_optional_done() {
        for ending in ["\n", "\r\n", "\r"] {
            for name in ["", "event: message\n", "event: response.completed\n"] {
                let body = format!("\u{feff}: keepalive\nid: fixture\nretry: 500\n{name}data: {{\n data-ignored: unused\ndata: \"type\":\"response.completed\",\n data-ignored: unused\ndata: \"response\":{{\"status\":\"completed\"}}}}\n\ndata: [DONE]\n\n").replace('\n', ending);
                assert_eq!(parse(body.as_bytes()).unwrap()["status"], "completed");
            }
        }
    }

    #[test]
    fn missing_terminal_or_blank_line_never_succeeds() {
        for stream in [
            "".into(),
            ": keepalive\n\n".into(),
            "data: [DONE]\n\n".into(),
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"21颗\"}\n\n".into(),
            completed().trim_end().to_owned(),
            completed().trim_end_matches('\n').to_owned() + "\n",
        ] {
            assert_eq!(parse(stream.as_bytes()).unwrap_err().kind, "incomplete");
        }
    }

    #[test]
    fn damaged_event_reports_event_local_json_position() {
        let stream = ": comment\n\ndata: {\"type\":\"response.created\"}\n\ndata: {\n data-ignored: unused\ndata: \"type\":\n\n";
        let failure = parse(stream.as_bytes()).unwrap_err();
        assert_eq!(failure.kind, "invalid_json");
        assert_eq!(failure.event_index, 2);
        let error = failure.json_error.unwrap();
        assert_eq!(error.line(), 2);
        assert!(error.column() > 0);
    }

    #[test]
    fn contradictory_names_ids_and_terminals_are_rejected() {
        let valid = completed();
        for stream in [
            valid.replace("event: response.completed", "event: response.created"),
            format!("data: {{\"type\":\"response.created\",\"response\":{{\"id\":\"other\"}}}}\n\n{valid}"),
            format!("{valid}{valid}"),
            format!("{valid}data: [DONE]\n\ndata: {{\"type\":\"metadata\"}}\n\n"),
            "data: {\"response\":{}}\n\n".into(),
            "data: {\"type\":\"response.completed\"}\n\n".into(),
        ] {
            assert_eq!(parse(stream.as_bytes()).unwrap_err().kind, "invalid_event_stream");
        }
    }

    #[test]
    fn failure_events_even_after_completed_never_judge() {
        for kind in [
            "error",
            "response.error",
            "response.failed",
            "response.incomplete",
        ] {
            for prefix in [String::new(), completed()] {
                let stream = format!("{prefix}data: {{\"type\":\"{kind}\"}}\n\n");
                assert_eq!(parse(stream.as_bytes()).unwrap_err().kind, "incomplete");
            }
        }
    }

    #[test]
    fn metadata_tail_allowed_but_incomplete_response_still_rejected() {
        let stream = format!(
            "{}data: {{\"type\":\"usage\",\"tokens\":1}}\n\n: final comment\n",
            completed()
        );
        assert!(parse(stream.as_bytes()).is_ok());
        let result = crate::api::parse_response(
            &parse(
                completed()
                    .replace("\"status\":\"completed\"", "\"status\":\"incomplete\"")
                    .as_bytes(),
            )
            .unwrap(),
        );
        assert!(result.error.is_some());
    }

    #[test]
    fn detection_is_bounded_to_mime_or_recognizable_prefix() {
        assert!(is_event_stream(
            Some(" Text/Event-Stream; charset=utf-8"),
            b""
        ));
        assert!(is_event_stream(None, b"\xef\xbb\xbf: keepalive\n"));
        assert!(is_event_stream(None, b"data: {}\n\n"));
        assert!(!is_event_stream(None, b"{\"status\":\"completed\"}"));
        assert!(!is_event_stream(None, b"proxy error"));
        assert!(!is_event_stream(Some("application/json"), &[0xff]));
        assert_eq!(parse(&[0xff]).unwrap_err().kind, "invalid_event_stream");
    }
}

impl StreamFailure {
    fn invalid(message: &'static str, event_index: usize) -> Self {
        Self {
            kind: "invalid_event_stream",
            message,
            json_error: None,
            event_index,
        }
    }

    fn incomplete() -> Self {
        Self {
            kind: "incomplete",
            message: "Responses SSE 未收到完整结束事件，未执行判定。",
            json_error: None,
            event_index: 0,
        }
    }
}

/// Content-Type 忽略大小写及参数；网关头错误时，仅在正文以 SSE 字段/注释起始时尝试兼容。
/// 该识别不意味着成功，后续仍需完整事件边界、合法 JSON 和唯一 response.completed。
pub fn is_event_stream(content_type: Option<&str>, bytes: &[u8]) -> bool {
    if content_type.is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("text/event-stream"))
    }) {
        return true;
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let text = text.strip_prefix('\u{feff}').unwrap_or(text).trim_start();
    [":", "event:", "data:", "id:", "retry:"]
        .iter()
        .any(|prefix| text.starts_with(prefix))
}

#[derive(Default)]
struct StreamState {
    response_id: Option<String>,
    completed: Option<Value>,
    done: bool,
}

/// 解析已读到 EOF 且未超限的原始正文，支持 LF/CRLF/CR、UTF-8 BOM、多行 data 和保活注释。
/// 仅临时解析副本归一换行，诊断缓冲不改写；未以空行结束的数据事件不能当成完整事件。
/// 返回原样的 completed.response，调用方必须继续调用既有 parse_response 检查完整助手正文。
pub fn parse(bytes: &[u8]) -> Result<Value, StreamFailure> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| StreamFailure::invalid("Responses SSE 不是有效的 UTF-8，未执行判定。", 0))?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut state = StreamState::default();
    let mut event_name = "";
    let mut data = String::new();
    let mut has_data = false;
    let mut event_index = 0;
    // split_terminator 不把最后一个行结束符伪装为空行，避免 EOF 截断也触发结束事件。
    for line in normalized.split_terminator('\n') {
        if line.is_empty() {
            if has_data {
                data.pop();
                event_index += 1;
                dispatch(event_name, &data, event_index, &mut state)?;
            }
            event_name = "";
            data.clear();
            has_data = false;
            continue;
        }
        if line.starts_with(':') {
            continue;
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "event" => event_name = value,
            "data" => {
                data.push_str(value);
                data.push('\n');
                has_data = true;
            }
            // id/retry 与未知 SSE 字段不参与答案，也不触发重连或额外请求。
            _ => {}
        }
    }
    if has_data {
        return Err(StreamFailure::incomplete());
    }
    state.completed.ok_or_else(StreamFailure::incomplete)
}

/// 每个完整数据事件单独解析；允许结束后的用量/元数据，拒绝错误、矛盾编号与重复终态。
/// [DONE] 只作可选尾标记，不能代替 Responses 的明确结束，也不能忽略它之后的数据。
fn dispatch(
    event_name: &str,
    data: &str,
    event_index: usize,
    state: &mut StreamState,
) -> Result<(), StreamFailure> {
    if state.done {
        return Err(StreamFailure::invalid(
            "Responses SSE 结束标记后仍有数据，未执行判定。",
            event_index,
        ));
    }
    if data == "[DONE]" {
        if state.completed.is_none() {
            return Err(StreamFailure::incomplete());
        }
        state.done = true;
        return Ok(());
    }
    let value: Value = serde_json::from_str(data).map_err(|error| StreamFailure {
        kind: "invalid_json",
        message: "Responses SSE 事件不是有效的 JSON，未执行判定。",
        json_error: Some(error),
        event_index,
    })?;
    let Some(kind) = value.get("type").and_then(Value::as_str) else {
        return Err(StreamFailure::invalid(
            "Responses SSE 事件缺少有效类型，未执行判定。",
            event_index,
        ));
    };
    if !event_name.is_empty() && event_name != "message" && event_name != kind {
        return Err(StreamFailure::invalid(
            "Responses SSE 事件名称与正文类型不一致，未执行判定。",
            event_index,
        ));
    }
    if let Some(id) = value
        .get("response")
        .and_then(|response| response.get("id"))
    {
        let Some(id) = id.as_str().filter(|id| !id.is_empty()) else {
            return Err(StreamFailure::invalid(
                "Responses SSE 响应标识无效，未执行判定。",
                event_index,
            ));
        };
        if state
            .response_id
            .as_deref()
            .is_some_and(|saved| saved != id)
        {
            return Err(StreamFailure::invalid(
                "Responses SSE 混合多个响应，未执行判定。",
                event_index,
            ));
        }
        if state.response_id.is_none() {
            state.response_id = Some(id.to_owned());
        }
    }
    match kind {
        "error" | "response.error" | "response.failed" | "response.incomplete" => {
            Err(StreamFailure {
                kind: "incomplete",
                message: "Responses SSE 返回失败或未完成事件，未执行判定。",
                json_error: None,
                event_index,
            })
        }
        "response.completed" => {
            if state.completed.is_some() {
                return Err(StreamFailure::invalid(
                    "Responses SSE 重复返回结束事件，未执行判定。",
                    event_index,
                ));
            }
            let Some(response) = value
                .get("response")
                .filter(|response| response.is_object())
            else {
                return Err(StreamFailure::invalid(
                    "Responses SSE 结束事件缺少完整响应对象，未执行判定。",
                    event_index,
                ));
            };
            state.completed = Some(response.clone());
            Ok(())
        }
        // 增量、推理和元数据均不拼入正文；完整 response 内的最终消息仍由既有规则验证。
        _ => Ok(()),
    }
}
