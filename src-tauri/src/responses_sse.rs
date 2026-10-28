//! Responses SSE 的有界正文解析；网络读取、超时和取消仍由 api.rs 统一管理。
//! 只提取明确结束事件内的完整 response，不用 delta 拼答案，不执行正文中的任何指令。作者：ahui。

use serde_json::Value;

/// 固定用户提示与可选 JSON 位置；错误中不回显事件原文、服务端 message 或敏感元数据。
#[derive(Debug)]
pub struct StreamFailure {
    pub kind: &'static str,
    pub message: &'static str,
    pub json_error: Option<serde_json::Error>,
    /// 从 1 开始的数据事件序号；0 表示无法定位具体事件的流级错误。
    /// Chat 在 EOF 缺少结束证据时沿用最后完整事件序号，类型/实际 ID 仍为空。
    pub event_index: usize,
    /// 仅定位协议结构错误，不把事件正文或服务商自由文本带入用户提示。
    pub event_type: Option<String>,
    pub expected_response_id: Option<String>,
    pub actual_response_id: Option<String>,
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

    /// 合成两个不同响应的边界，不使用供应商 ID 前缀或真实题答来满足兼容条件。
    fn empty_prelude_events() -> Vec<Value> {
        vec![
            json!({"type":"response.created","response":{"id":"temporary","object":"response","status":"in_progress","output":[]}}),
            json!({"type":"response.in_progress","response":{"id":"temporary","object":"response","status":"in_progress","output":[]}}),
            json!({"type":"response.output_item.added","output_index":0,"item":{"id":"empty-message","type":"message","role":"assistant","status":"in_progress","content":[]}}),
            json!({"type":"response.content_part.added","item_id":"empty-message","output_index":0,"content_index":0,"part":{"type":"output_text","text":"","annotations":[]}}),
            json!({"type":"response.output_text.delta","item_id":"empty-message","output_index":0,"content_index":0,"delta":""}),
        ]
    }

    /// 前导之后的新 created 与唯一 completed 属于同一正式 ID，答案仍由完整终态验证。
    fn stream_after_prelude(events: &[Value]) -> String {
        let mut stream = String::from(": keepalive\n\n");
        for event in events {
            stream.push_str(&format!("data: {event}\n\n"));
        }
        stream.push_str("data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_fixture\",\"object\":\"response\",\"status\":\"in_progress\",\"output\":[]}}\n\n");
        stream.push_str(&completed());
        stream
    }

    #[test]
    fn validated_empty_prelude_switches_once_without_changing_final_answer() {
        for numbered in [false, true] {
            let mut events = empty_prelude_events();
            if numbered {
                for (index, event) in events.iter_mut().enumerate() {
                    event["sequence_number"] = json!(index);
                    event["response_id"] = json!("temporary");
                }
            }
            let response = parse(stream_after_prelude(&events).as_bytes()).unwrap();
            assert_eq!(response["id"], "resp_fixture");
            let result = crate::api::parse_response(&response);
            assert_eq!(result.answer, "21颗");
            assert!(result.error.is_none());
        }
    }

    #[test]
    fn prelude_cannot_discard_content_unknown_fields_or_broken_links() {
        // 空白也属于真实正文；索引、消息关联、角色或未知字段均不能按“看起来空”放行。
        for (index, pointer, replacement) in [
            (
                0,
                "/response/output",
                json!([{"type":"reasoning","summary":[]}]),
            ),
            (2, "/output_index", json!(1)),
            (2, "/item/role", json!("user")),
            (3, "/item_id", json!("unrelated-message")),
            (3, "/content_index", json!(1)),
            (3, "/part/text", json!("正文")),
            (3, "/part/annotations", json!([{}])),
            (4, "/delta", json!(" ")),
        ] {
            let mut events = empty_prelude_events();
            *events[index].pointer_mut(pointer).unwrap() = replacement;
            assert!(
                parse(stream_after_prelude(&events).as_bytes()).is_err(),
                "{pointer}"
            );
        }
        for extra in [
            json!({"type":"unknown","text":""}),
            json!({"type":"response.reasoning_text.delta","delta":"推理"}),
            json!({"type":"response.refusal.delta","delta":"拒绝"}),
            json!({"type":"response.output_item.added","output_index":1,"item":{"type":"function_call","arguments":"{}"}}),
            json!({"type":"response.completed","response":{"id":"temporary","status":"completed","output":[]}}),
            json!({"type":"response.failed"}),
        ] {
            let mut events = empty_prelude_events();
            events.push(extra);
            assert!(parse(stream_after_prelude(&events).as_bytes()).is_err());
        }
        let mut events = empty_prelude_events();
        events[4]["unexpected"] = json!({"text":"旁路输出"});
        assert!(parse(stream_after_prelude(&events).as_bytes()).is_err());
        events = empty_prelude_events();
        events[0]["sequence_number"] = json!(0);
        events[1]["sequence_number"] = json!(2);
        assert!(parse(stream_after_prelude(&events).as_bytes()).is_err());
    }

    #[test]
    fn second_switch_and_wrong_terminal_keep_id_failure_context() {
        let valid = stream_after_prelude(&empty_prelude_events());
        let third = "data: {\"type\":\"response.created\",\"response\":{\"id\":\"third\",\"object\":\"response\",\"status\":\"in_progress\",\"output\":[]}}\n\n";
        for suffix in [
            format!("{third}{}", completed()),
            completed().replace("resp_fixture", "third"),
        ] {
            let source = valid.replace(&completed(), &suffix);
            let failure = parse(source.as_bytes()).unwrap_err();
            assert_eq!(failure.kind, "invalid_event_stream");
            assert_eq!(failure.event_index, 7);
            assert_eq!(
                failure.expected_response_id.as_deref(),
                Some("resp_fixture")
            );
            assert_eq!(failure.actual_response_id.as_deref(), Some("third"));
            assert!(failure.json_error.is_none());
        }
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
            event_type: None,
            expected_response_id: None,
            actual_response_id: None,
        }
    }

    fn incomplete() -> Self {
        Self {
            kind: "incomplete",
            message: "Responses SSE 未收到完整结束事件，未执行判定。",
            json_error: None,
            event_index: 0,
            event_type: None,
            expected_response_id: None,
            actual_response_id: None,
        }
    }

    /// 只在失败时解析定位副本；原始字节保持不变，保存前还会进行凭据保护和长度限制。
    pub fn with_context(mut self, event: &str, data: &str, expected: Option<&str>) -> Self {
        let value = serde_json::from_str::<Value>(data).ok();
        self.event_type = value
            .as_ref()
            .and_then(|value| value["type"].as_str().or_else(|| value["object"].as_str()))
            .or((!event.is_empty()).then_some(event))
            .map(str::to_owned);
        self.expected_response_id = expected.map(str::to_owned);
        self.actual_response_id = value
            .as_ref()
            .and_then(|value| {
                value["response"]["id"]
                    .as_str()
                    .or_else(|| value["response_id"].as_str())
                    .or_else(|| value["id"].as_str())
            })
            .map(str::to_owned);
        self
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
    /// 只允许丢弃一段已经逐事件证明为空的前导；正文仍仅从正式 completed 提取。
    prelude: EmptyPrelude,
    switched: bool,
}

/// 兼容范围来自已核实的空 created/in_progress → 空消息 → 空文本事件。
/// 未知字段、未知事件、任何语义输出或终态都会永久关闭切换资格，不能靠后续空块重新打开。
#[derive(Default)]
struct EmptyPrelude {
    created: bool,
    blocked: bool,
    message_id: Option<String>,
    part_added: bool,
    sequence: Option<u64>,
}

/// 仅白名单结构可以证明“空”；不能凭 ID 前缀、缺失 text 或笼统的空值判断。
fn only_keys(value: &Value, keys: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|object| object.keys().all(|key| keys.contains(&key.as_str())))
}

/// 临时前导的生命周期快照只接受已观察到的字段；正式 created 可以携带标准配置元数据，
/// 但必须明确 output=[]、in_progress，且没有错误或未完成信息。
fn empty_lifecycle(value: &Value, temporary: bool) -> bool {
    value["object"] == "response"
        && value["status"] == "in_progress"
        && value["id"].as_str().is_some_and(|id| !id.is_empty())
        && value["output"].as_array().is_some_and(Vec::is_empty)
        && value.get("error").is_none_or(Value::is_null)
        && value.get("incomplete_details").is_none_or(Value::is_null)
        && (if temporary {
            only_keys(
                value,
                &[
                    "id",
                    "object",
                    "status",
                    "output",
                    "model",
                    "created_at",
                    "error",
                    "incomplete_details",
                ],
            )
        } else {
            // 正式 created 的这些字段是配置/用量元数据；未知附加结构不能证明没有旁路输出。
            only_keys(
                value,
                &[
                    "id",
                    "object",
                    "status",
                    "output",
                    "model",
                    "created_at",
                    "error",
                    "incomplete_details",
                    "access_programs",
                    "background",
                    "completed_at",
                    "frequency_penalty",
                    "instructions",
                    "max_output_tokens",
                    "max_tool_calls",
                    "metadata",
                    "moderation",
                    "parallel_tool_calls",
                    "presence_penalty",
                    "previous_response_id",
                    "prompt_cache_key",
                    "prompt_cache_retention",
                    "reasoning",
                    "safety_identifier",
                    "service_tier",
                    "store",
                    "temperature",
                    "text",
                    "tool_choice",
                    "tool_usage",
                    "tools",
                    "top_logprobs",
                    "top_p",
                    "truncation",
                    "usage",
                    "user",
                ],
            )
        })
}

impl EmptyPrelude {
    /// 每个空文本事件必须指向已添加的同一消息和 content_index=0；sequence 若提供须连续。
    /// 不要求 ID 具有任何供应商特有前缀，也不读取配置/题目判断该响应是否可以丢弃。
    fn observe(&mut self, kind: &str, value: &Value) {
        if self.blocked {
            return;
        }
        let sequence = value.get("sequence_number");
        if sequence.is_some_and(|number| number.as_u64().is_none())
            || self
                .sequence
                .is_some_and(|previous| sequence.and_then(Value::as_u64) != previous.checked_add(1))
        {
            self.blocked = true;
            return;
        }
        self.sequence = sequence.and_then(Value::as_u64);
        let common = ["type", "sequence_number", "response_id"];
        let allowed = |extra: &[&str]| only_keys(value, &[common.as_slice(), extra].concat());
        let linked = || {
            value["output_index"].as_u64() == Some(0)
                && value["item_id"]
                    .as_str()
                    .is_some_and(|id| self.message_id.as_deref() == Some(id))
        };
        let empty = match kind {
            "response.created" => {
                let valid = !self.created
                    && allowed(&["response"])
                    && empty_lifecycle(&value["response"], true);
                self.created = true;
                valid
            }
            "response.in_progress" => {
                self.created && allowed(&["response"]) && empty_lifecycle(&value["response"], true)
            }
            "response.output_item.added" => {
                let item = &value["item"];
                let valid = self.created
                    && self.message_id.is_none()
                    && allowed(&["item", "output_index"])
                    && value["output_index"].as_u64() == Some(0)
                    && only_keys(item, &["type", "id", "role", "status", "content"])
                    && item["type"] == "message"
                    && item["role"] == "assistant"
                    && item["status"] == "in_progress"
                    && item["content"].as_array().is_some_and(Vec::is_empty)
                    && item["id"].as_str().is_some_and(|id| !id.is_empty());
                if valid {
                    self.message_id = item["id"].as_str().map(str::to_owned);
                }
                valid
            }
            "response.content_part.added" => {
                let part = &value["part"];
                let valid = !self.part_added
                    && linked()
                    && value["content_index"].as_u64() == Some(0)
                    && allowed(&["part", "output_index", "content_index", "item_id"])
                    && only_keys(part, &["type", "text", "annotations"])
                    && part["type"] == "output_text"
                    && part["text"] == ""
                    && part["annotations"].as_array().is_some_and(Vec::is_empty);
                self.part_added = valid;
                valid
            }
            "response.output_text.delta" => {
                self.part_added
                    && linked()
                    && value["content_index"].as_u64() == Some(0)
                    && allowed(&["delta", "output_index", "content_index", "item_id"])
                    && value["delta"] == ""
            }
            _ => false,
        };
        self.blocked = !empty;
    }
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
                dispatch(event_name, &data, event_index, &mut state).map_err(|failure| {
                    failure.with_context(event_name, &data, state.response_id.as_deref())
                })?;
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
        event_type: None,
        expected_response_id: None,
        actual_response_id: None,
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
    let nested_id = value
        .get("response")
        .and_then(|response| response.get("id"));
    let explicit_id = value.get("response_id");
    if nested_id.is_some() && explicit_id.is_some() && nested_id != explicit_id {
        return Err(StreamFailure::invalid(
            "Responses SSE 事件关联的响应标识矛盾，未执行判定。",
            event_index,
        ));
    }
    if let Some(id) = nested_id.or(explicit_id) {
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
            // 只能在新的 created 边界切换一次；此前所有数据事件都必须有可验证的空结构。
            // 正式响应增量不拼接进答案，丢弃前导也不会把不同响应的正文接在一起。
            if !state.switched
                && kind == "response.created"
                && state.prelude.created
                && !state.prelude.blocked
                && state.completed.is_none()
                && empty_lifecycle(&value["response"], false)
                && only_keys(
                    &value,
                    &["type", "sequence_number", "response", "response_id"],
                )
                && value
                    .get("sequence_number")
                    .is_none_or(|number| number.as_u64().is_some())
            {
                state.response_id = Some(id.to_owned());
                state.switched = true;
            } else {
                return Err(StreamFailure::invalid(
                    "Responses SSE 混合多个响应或前导无法证明为空，未执行判定。",
                    event_index,
                ));
            }
        }
        if state.response_id.is_none() {
            state.response_id = Some(id.to_owned());
        }
    }
    if !state.switched {
        state.prelude.observe(kind, &value);
    }
    match kind {
        "error" | "response.error" | "response.failed" | "response.incomplete" => {
            Err(StreamFailure {
                kind: "incomplete",
                message: "Responses SSE 返回失败或未完成事件，未执行判定。",
                json_error: None,
                event_index,
                event_type: None,
                expected_response_id: None,
                actual_response_id: None,
            })
        }
        "response.completed" => {
            if state.switched && nested_id.is_none() {
                return Err(StreamFailure::invalid(
                    "Responses SSE 正式结束事件缺少响应标识，未执行判定。",
                    event_index,
                ));
            }
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
