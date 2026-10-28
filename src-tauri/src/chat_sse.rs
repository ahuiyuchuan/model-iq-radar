//! Chat Completions SSE 解析：只还原同一响应、index=0 的助手正文，不混入推理或工具参数。
//! 调用方须先在共享期限和 2 MiB 上限内读到 EOF；原始字节不改写，不执行模型指令。
//! 返回兼容既有 parse_completion 的对象，继续使用原来的完整性、空回答与判定规则。作者：ahui。

use crate::responses_sse::StreamFailure;
use serde_json::{json, Value};

/// 每次解析独立持有，正文只追加一次；finish_reason 与 [DONE] 两者齐全才构成完整流。
#[derive(Default)]
struct StreamState {
    response_id: Option<String>,
    answer: String,
    finish_reason: Option<String>,
    done: bool,
}

/// 解析已完整读取的 SSE；兼容 BOM、LF/CRLF/CR、多行 data 和保活注释。
/// 仅解析副本归一换行，增量 content 字符串本身保留原样；不拼接 reasoning 等旁路字段。
/// 返回的标准消息仍须经过 parse_completion，非 stop 结束、空正文等不能判成功。
pub fn parse(bytes: &[u8]) -> Result<Value, StreamFailure> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| invalid("Chat Completions SSE 不是有效的 UTF-8，未执行判定。", 0))?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut state = StreamState::default();
    let mut event_name = "";
    let mut data = String::new();
    let mut has_data = false;
    let mut event_index = 0;
    // 不把最后一个换行伪装为空行：未完整封闭的 data 事件不能作为有效终态。
    for line in normalized.split_terminator('\n') {
        if line.is_empty() {
            if has_data {
                data.pop();
                event_index += 1;
                // Chat 尚无真实失败原文；继续严格拒绝 ID 变化，仅补齐失败事件定位。
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
            // id/retry 等 SSE 字段不触发重连；响应标识仅以 JSON chunk.id 为准。
            _ => {}
        }
    }
    if has_data || !state.done || state.finish_reason.is_none() {
        return Err(incomplete(event_index));
    }
    // 兼容现有 JSON 解析器允许省略 role 的网关；流中若显式提供 role，已逐块校验为 assistant。
    Ok(json!({"choices":[{
        "index":0,
        "finish_reason":state.finish_reason,
        "message":{"role":"assistant", "content":state.answer}
    }]}))
}

/// 检查单个完整事件；拒绝混合响应、多候选、重复终态及结束后的正文。
/// 完成后允许标准 choices=[] 用量块；只有 [DONE]、只有正文或只有 stop 都不能替代完整流。
fn dispatch(
    event_name: &str,
    data: &str,
    event_index: usize,
    state: &mut StreamState,
) -> Result<(), StreamFailure> {
    if state.done {
        return Err(invalid(
            "Chat Completions SSE 结束标记后仍有数据，未执行判定。",
            event_index,
        ));
    }
    if event_name == "error" {
        return Err(invalid(
            "Chat Completions SSE 返回错误事件，未执行判定。",
            event_index,
        ));
    }
    if !event_name.is_empty() && event_name != "message" {
        return Err(invalid(
            "Chat Completions SSE 事件类型不受支持，未执行判定。",
            event_index,
        ));
    }
    if data == "[DONE]" {
        if state.finish_reason.is_none() {
            return Err(incomplete(event_index));
        }
        state.done = true;
        return Ok(());
    }
    let value: Value = serde_json::from_str(data).map_err(|error| StreamFailure {
        kind: "invalid_json",
        message: "Chat Completions SSE 事件不是有效的 JSON，未执行判定。",
        json_error: Some(error),
        event_index,
        event_type: None,
        expected_response_id: None,
        actual_response_id: None,
    })?;
    if value.get("error").is_some_and(|error| !error.is_null()) {
        return Err(invalid(
            "Chat Completions SSE 返回接口错误，未执行判定。",
            event_index,
        ));
    }
    if value["object"] != "chat.completion.chunk" {
        return Err(invalid(
            "Chat Completions SSE 缺少有效的分块类型，未执行判定。",
            event_index,
        ));
    }
    let Some(id) = value
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
    else {
        return Err(invalid(
            "Chat Completions SSE 响应标识无效，未执行判定。",
            event_index,
        ));
    };
    if state
        .response_id
        .as_deref()
        .is_some_and(|saved| saved != id)
    {
        return Err(invalid(
            "Chat Completions SSE 混合多个响应，未执行判定。",
            event_index,
        ));
    }
    if state.response_id.is_none() {
        state.response_id = Some(id.to_owned());
    }
    let Some(choices) = value.get("choices").and_then(Value::as_array) else {
        return Err(invalid(
            "Chat Completions SSE 未返回有效 choices，未执行判定。",
            event_index,
        ));
    };
    if choices.is_empty() {
        // 官方 include_usage 的尾块 choices 为空数组；不能把任意空块当作结束，也不拼入正文。
        if state.finish_reason.is_some() && value.get("usage").is_some_and(Value::is_object) {
            return Ok(());
        }
        return Err(invalid(
            "Chat Completions SSE 空候选块缺少有效用量尾部，未执行判定。",
            event_index,
        ));
    }
    if choices.len() != 1 || choices[0]["index"].as_u64() != Some(0) {
        return Err(invalid(
            "Chat Completions SSE 返回多个或无效候选，未执行判定。",
            event_index,
        ));
    }
    if state.finish_reason.is_some() {
        return Err(invalid(
            "Chat Completions SSE 完成后仍返回候选数据，未执行判定。",
            event_index,
        ));
    }
    let choice = &choices[0];
    let Some(delta) = choice.get("delta").filter(|delta| delta.is_object()) else {
        return Err(invalid(
            "Chat Completions SSE 缺少有效 delta，未执行判定。",
            event_index,
        ));
    };
    if delta
        .get("role")
        .is_some_and(|role| !role.is_null() && role != "assistant")
    {
        return Err(invalid(
            "Chat Completions SSE 返回非助手消息，未执行判定。",
            event_index,
        ));
    }
    if delta.get("refusal").is_some_and(|value| !value.is_null()) {
        return Err(StreamFailure {
            kind: "filtered",
            message: "接口返回拒绝回答，未执行判定。",
            json_error: None,
            event_index,
            event_type: None,
            expected_response_id: None,
            actual_response_id: None,
        });
    }
    if delta
        .get("function_call")
        .is_some_and(|value| !value.is_null())
        || delta.get("tool_calls").is_some_and(|value| {
            !value.is_null() && value.as_array().is_none_or(|items| !items.is_empty())
        })
    {
        return Err(StreamFailure {
            kind: "tool_call",
            message: "模型请求调用工具，本测试不执行工具。",
            json_error: None,
            event_index,
            event_type: None,
            expected_response_id: None,
            actual_response_id: None,
        });
    }
    if let Some(content) = delta.get("content").filter(|value| !value.is_null()) {
        let Some(text) = content.as_str() else {
            return Err(invalid(
                "Chat Completions SSE 正文不是文本，未执行判定。",
                event_index,
            ));
        };
        state.answer.push_str(text);
    }
    // 推理、用量和 obfuscation 等元数据均不参与答案；无合法最终正文时由原解析器拒绝。
    if let Some(reason) = choice.get("finish_reason").filter(|value| !value.is_null()) {
        let Some(reason) = reason.as_str() else {
            return Err(invalid(
                "Chat Completions SSE 结束原因无效，未执行判定。",
                event_index,
            ));
        };
        state.finish_reason = Some(reason.to_owned());
    }
    Ok(())
}

/// 固定错误说明不带服务商原文；事件序号仅用于定位，不根据报错猜测服务器内部原因。
fn invalid(message: &'static str, event_index: usize) -> StreamFailure {
    StreamFailure {
        kind: "invalid_event_stream",
        message,
        json_error: None,
        event_index,
        event_type: None,
        expected_response_id: None,
        actual_response_id: None,
    }
}

/// 流结束证据不全时统一拒绝，不以已收到的关键词或 HTML 增量提前结算成功。
fn incomplete(event_index: usize) -> StreamFailure {
    StreamFailure {
        kind: "incomplete",
        message: "Chat Completions SSE 未收到完整结束状态和结束标记，未执行判定。",
        json_error: None,
        event_index,
        event_type: None,
        expected_response_id: None,
        actual_response_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 只用虚构 chunk，正文中的换行/空格必须原样保留，绝不使用真实取证数据。
    fn chunk(delta: Value, reason: Value) -> String {
        format!(
            "data: {}\n\n",
            json!({
                "id":"chat-fixture", "object":"chat.completion.chunk",
                "choices":[{"index":0,"delta":delta,"finish_reason":reason}]
            })
        )
    }

    fn stream(reason: &str) -> String {
        format!(
            "{}{}{}data: [DONE]\n\n",
            chunk(
                json!({"role":"assistant","reasoning_content":"不能混入正文"}),
                Value::Null
            ),
            chunk(json!({"content":"  <svg>\n原文</svg>  "}), Value::Null),
            chunk(json!({}), json!(reason))
        )
    }

    #[test]
    fn complete_text_preserves_whitespace_and_ignores_reasoning_and_usage() {
        let usage = format!(
            "data: {}\n\n",
            json!({
                "id":"chat-fixture", "object":"chat.completion.chunk", "choices":[], "usage":{"total_tokens":1}
            })
        );
        for ending in ["\n", "\r\n", "\r"] {
            let source = format!(
                "\u{feff}: keepalive\n\n{}",
                stream("stop").replace("data: [DONE]", &format!("{usage}data: [DONE]"))
            )
            .replace('\n', ending);
            let result = crate::api::parse_completion(&parse(source.as_bytes()).unwrap());
            assert!(result.error.is_none());
            assert_eq!(result.answer, "  <svg>\n原文</svg>  ");
        }
    }

    #[test]
    fn missing_end_markers_and_data_after_terminal_never_succeed() {
        let valid = stream("stop");
        for source in [
            valid.replace("data: [DONE]\n\n", ""),
            valid.trim_end_matches('\n').to_owned() + "\n",
            "data: [DONE]\n\n".into(),
            format!("{valid}{}", chunk(json!({"content":"extra"}), Value::Null)),
            valid.replace(
                "data: [DONE]",
                &format!("{}data: [DONE]", chunk(json!({}), json!("stop"))),
            ),
        ] {
            assert!(parse(source.as_bytes()).is_err());
        }
        let incomplete = crate::api::parse_completion(&parse(stream("length").as_bytes()).unwrap());
        assert_eq!(incomplete.error_kind.as_deref(), Some("incomplete"));
        assert_eq!(incomplete.answer, "  <svg>\n原文</svg>  ");
    }

    #[test]
    fn mixed_responses_roles_and_candidates_are_rejected() {
        let valid = stream("stop");
        let changed_id = valid.replacen("chat-fixture", "different-response", 1);
        // 即便第一块只是角色，也不在缺少真实失败证据时放宽 ID 校验；诊断须指出两侧 ID。
        let failure = parse(changed_id.as_bytes()).unwrap_err();
        assert_eq!(
            failure.expected_response_id.as_deref(),
            Some("different-response")
        );
        assert_eq!(failure.actual_response_id.as_deref(), Some("chat-fixture"));
        assert_eq!(failure.event_type.as_deref(), Some("chat.completion.chunk"));
        assert_eq!(failure.event_index, 2);
        for source in [
            changed_id,
            valid.replace("\"assistant\"", "\"user\""),
            valid.replace("\"index\":0", "\"index\":1"),
            "data: {\"type\":\"response.completed\",\"response\":{}}\n\n".into(),
        ] {
            assert_eq!(
                parse(source.as_bytes()).unwrap_err().kind,
                "invalid_event_stream"
            );
        }
        let malformed = parse(b"data: {\n\n").unwrap_err();
        assert_eq!(malformed.kind, "invalid_json");
        assert_eq!(malformed.event_index, 1);
        assert!(malformed.json_error.is_some());
    }

    #[test]
    fn refusal_tools_and_reasoning_only_never_become_successful_answers() {
        for (delta, expected) in [
            (json!({"refusal":"no"}), "filtered"),
            (json!({"tool_calls":[{"index":0}]}), "tool_call"),
            (json!({"function_call":{"name":"fixture"}}), "tool_call"),
        ] {
            assert_eq!(
                parse(chunk(delta, Value::Null).as_bytes())
                    .unwrap_err()
                    .kind,
                expected
            );
        }
        let source = format!(
            "{}data: [DONE]\n\n",
            chunk(json!({"reasoning_content":"21颗"}), json!("stop"))
        );
        let result = crate::api::parse_completion(&parse(source.as_bytes()).unwrap());
        assert_eq!(result.error_kind.as_deref(), Some("empty_response"));
    }

    #[test]
    fn multiline_data_is_supported_but_unknown_events_and_multiple_choices_are_rejected() {
        let valid = stream("stop");
        // 在 JSON 字段之间换行，而不是改写 content 字符串；多行 data 按 SSE 规范连接。
        let multiline = valid.replace("\"object\":", "\ndata: \"object\":");
        assert_eq!(
            parse(multiline.as_bytes()).unwrap()["choices"][0]["message"]["content"],
            "  <svg>\n原文</svg>  "
        );
        for source in [
            format!("event: unknown\n{valid}"),
            valid.replacen("\"index\":0", "\"index\":-1", 1),
            valid.replacen("\"choices\":[", "\"choices\":[{},", 1),
            valid.replacen(
                "\"content\":\"  <svg>\\n原文</svg>  \"",
                "\"content\":[]",
                1,
            ),
        ] {
            assert_eq!(
                parse(source.as_bytes()).unwrap_err().kind,
                "invalid_event_stream"
            );
        }
    }
}
