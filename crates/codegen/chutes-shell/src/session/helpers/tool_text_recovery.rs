//! Recover tool calls a model emitted as plain text.
//!
//! A chute serves an open-weight model through vLLM or SGLang, and the
//! server-side tool-call parser is part of *that deployment's* configuration,
//! not of the model. When it is missing or mismatched, the model still emits a
//! perfectly well-formed call — in its own chat-template syntax, as assistant
//! text — and the `tool_calls` array arrives empty. The turn then ends having
//! done nothing, and the user sees the raw markup.
//!
//! This module recognizes those syntaxes. It is deliberately model-agnostic:
//! the formats belong to chat templates, several models share one, and a
//! per-model table would need an entry for every new generation.
//!
//! It only *finds candidates*. Whether a candidate becomes a real tool call is
//! the caller's decision, and it must require the name to resolve to a
//! registered tool and the arguments to pass that tool's own parser — this
//! module cannot tell a genuine call from a model quoting one in prose.

use std::ops::Range;

/// A tool call found in assistant text, and the span it occupied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveredToolCall {
    pub name: String,
    /// The arguments as a JSON object string, ready for the tool's parser.
    pub arguments: String,
    /// Byte range of the whole markup block in the source text.
    pub span: Range<usize>,
}

/// Hermes-style, used by Qwen and many fine-tunes.
const HERMES_OPEN: &str = "<tool_call>";
const HERMES_CLOSE: &str = "</tool_call>";
/// Kimi K2's template.
const KIMI_OPEN: &str = "<|tool_call_begin|>";
const KIMI_ARGS: &str = "<|tool_call_argument_begin|>";
const KIMI_CLOSE: &str = "<|tool_call_end|>";
/// Llama-style inline function syntax.
const LLAMA_OPEN: &str = "<function=";
const LLAMA_CLOSE: &str = "</function>";
/// A fenced JSON block. Accepted only under the strict shape below, because
/// unlike the tagged formats a fence is also how a model legitimately *shows*
/// JSON to the user.
const FENCE_OPEN: &str = "```json";
const FENCE_CLOSE: &str = "```";

/// Find every tool call encoded as text, ordered by position.
///
/// Overlapping matches cannot occur: each scan consumes its own block, and the
/// results are sorted and de-overlapped before returning.
pub fn find_tool_calls_in_text(text: &str) -> Vec<RecoveredToolCall> {
    let mut found = Vec::new();
    collect_tagged(text, HERMES_OPEN, HERMES_CLOSE, false, &mut found);
    collect_tagged(text, FENCE_OPEN, FENCE_CLOSE, true, &mut found);
    collect_kimi(text, &mut found);
    collect_llama(text, &mut found);

    found.sort_by_key(|c| c.span.start);
    // A fenced block nested inside a `<tool_call>` block would otherwise be
    // reported twice.
    let mut deduped: Vec<RecoveredToolCall> = Vec::with_capacity(found.len());
    for call in found {
        if deduped
            .last()
            .is_some_and(|prev| call.span.start < prev.span.end)
        {
            continue;
        }
        deduped.push(call);
    }
    deduped
}

/// Remove recovered blocks from `text`, so the malformed markup is not replayed
/// to the model on every later turn.
pub fn strip_recovered_spans(text: &str, calls: &[RecoveredToolCall]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    for call in calls {
        if call.span.start >= cursor {
            out.push_str(&text[cursor..call.span.start]);
            cursor = call.span.end;
        }
    }
    out.push_str(&text[cursor..]);
    out.trim().to_owned()
}

/// True unless the user turned recovery off.
pub fn recovery_enabled() -> bool {
    !std::env::var("CHUTES_DISABLE_TOOL_TEXT_RECOVERY")
        .ok()
        .is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
}

/// Scan for `open … close` pairs whose body is a `{"name":…, "arguments":…}`
/// object. With `strict`, the object must carry those two keys and nothing
/// else.
fn collect_tagged(
    text: &str,
    open: &str,
    close: &str,
    strict: bool,
    out: &mut Vec<RecoveredToolCall>,
) {
    let mut cursor = 0;
    while let Some((start, end, body)) = next_delimited(text, cursor, open, close) {
        cursor = end;
        if let Some((name, arguments)) = parse_name_and_arguments(body, strict) {
            out.push(RecoveredToolCall {
                name,
                arguments,
                span: start..end,
            });
        }
    }
}

/// `<|tool_call_begin|>functions.NAME:0<|tool_call_argument_begin|>{…}<|tool_call_end|>`
fn collect_kimi(text: &str, out: &mut Vec<RecoveredToolCall>) {
    let mut cursor = 0;
    while let Some((start, end, body)) = next_delimited(text, cursor, KIMI_OPEN, KIMI_CLOSE) {
        cursor = end;
        let Some((head, arguments)) = body.split_once(KIMI_ARGS) else {
            continue;
        };
        // `functions.get_weather:0` — strip the namespace and the call index.
        let name = head
            .trim()
            .rsplit_once(':')
            .map_or(head.trim(), |(name, index)| {
                if index.chars().all(|c| c.is_ascii_digit()) {
                    name
                } else {
                    head.trim()
                }
            })
            .trim_start_matches("functions.")
            .trim();
        if name.is_empty() || !is_json_object(arguments.trim()) {
            continue;
        }
        out.push(RecoveredToolCall {
            name: name.to_owned(),
            arguments: arguments.trim().to_owned(),
            span: start..end,
        });
    }
}

/// `<function=NAME>{…}</function>`
fn collect_llama(text: &str, out: &mut Vec<RecoveredToolCall>) {
    let mut cursor = 0;
    while let Some((start, end, body)) = next_delimited(text, cursor, LLAMA_OPEN, LLAMA_CLOSE) {
        cursor = end;
        let Some((name, arguments)) = body.split_once('>') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() || !is_json_object(arguments.trim()) {
            continue;
        }
        out.push(RecoveredToolCall {
            name: name.to_owned(),
            arguments: arguments.trim().to_owned(),
            span: start..end,
        });
    }
}

/// Next `open … close` pair at or after `from`, as `(block_start, block_end, body)`.
fn next_delimited<'a>(
    text: &'a str,
    from: usize,
    open: &str,
    close: &str,
) -> Option<(usize, usize, &'a str)> {
    if from >= text.len() {
        return None;
    }
    let start = text[from..].find(open)? + from;
    let body_start = start + open.len();
    let body_len = text[body_start..].find(close)?;
    let body_end = body_start + body_len;
    Some((start, body_end + close.len(), &text[body_start..body_end]))
}

/// Read `{"name": …, "arguments": …}`, tolerating `parameters` as the argument
/// key and arguments delivered as an already-serialized string.
fn parse_name_and_arguments(payload: &str, strict: bool) -> Option<(String, String)> {
    let value: serde_json::Value = serde_json::from_str(payload.trim()).ok()?;
    let obj = value.as_object()?;
    let name = obj.get("name")?.as_str()?.trim().to_owned();
    if name.is_empty() {
        return None;
    }
    let arguments = obj.get("arguments").or_else(|| obj.get("parameters"))?;
    // A fenced block only counts as a call when it is *exactly* a call — a
    // model showing an example payload usually carries more keys, or fewer.
    if strict && obj.len() != 2 {
        return None;
    }
    match arguments {
        serde_json::Value::String(serialized) => {
            is_json_object(serialized).then(|| (name, serialized.clone()))
        }
        serde_json::Value::Object(_) => Some((name, arguments.to_string())),
        _ => None,
    }
}

fn is_json_object(text: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(text).is_ok_and(|v| v.is_object())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(text: &str) -> RecoveredToolCall {
        let calls = find_tool_calls_in_text(text);
        assert_eq!(calls.len(), 1, "expected exactly one call in {text:?}");
        calls.into_iter().next().unwrap()
    }

    #[test]
    fn hermes_block_is_recovered() {
        let call = one(
            "<tool_call>\n{\"name\": \"read_file\", \"arguments\": {\"path\": \"a.rs\"}}\n</tool_call>",
        );
        assert_eq!(call.name, "read_file");
        assert_eq!(call.arguments, "{\"path\":\"a.rs\"}");
    }

    #[test]
    fn arguments_delivered_as_a_serialized_string_are_kept_verbatim() {
        let call = one(
            "<tool_call>{\"name\":\"bash\",\"arguments\":\"{\\\"cmd\\\":\\\"ls\\\"}\"}</tool_call>",
        );
        assert_eq!(call.arguments, "{\"cmd\":\"ls\"}");
    }

    #[test]
    fn parameters_is_accepted_as_the_argument_key() {
        let call = one("<tool_call>{\"name\":\"grep\",\"parameters\":{\"q\":\"x\"}}</tool_call>");
        assert_eq!(call.name, "grep");
        assert_eq!(call.arguments, "{\"q\":\"x\"}");
    }

    #[test]
    fn kimi_block_is_recovered_without_its_namespace_or_index() {
        let call = one(
            "<|tool_call_begin|>functions.list_dir:0<|tool_call_argument_begin|>{\"p\":\".\"}<|tool_call_end|>",
        );
        assert_eq!(call.name, "list_dir");
        assert_eq!(call.arguments, "{\"p\":\".\"}");
    }

    #[test]
    fn llama_inline_function_is_recovered() {
        let call = one("<function=write_file>{\"path\":\"a\"}</function>");
        assert_eq!(call.name, "write_file");
        assert_eq!(call.arguments, "{\"path\":\"a\"}");
    }

    #[test]
    fn a_fenced_block_is_recovered_only_in_the_exact_call_shape() {
        let call =
            one("Here:\n```json\n{\"name\":\"read_file\",\"arguments\":{\"path\":\"a\"}}\n```");
        assert_eq!(call.name, "read_file");

        // An example payload the model is merely showing the user.
        assert!(
            find_tool_calls_in_text(
                "```json\n{\"name\":\"read_file\",\"arguments\":{},\"note\":\"example\"}\n```"
            )
            .is_empty()
        );
        assert!(find_tool_calls_in_text("```json\n{\"path\":\"a.rs\"}\n```").is_empty());
    }

    #[test]
    fn prose_and_malformed_blocks_are_not_recovered() {
        assert!(find_tool_calls_in_text("I would call read_file with path a.rs").is_empty());
        // No closing tag: a truncated stream must not half-execute.
        assert!(find_tool_calls_in_text("<tool_call>{\"name\":\"x\",\"arguments\":{}}").is_empty());
        // Arguments that are not an object.
        assert!(
            find_tool_calls_in_text("<tool_call>{\"name\":\"x\",\"arguments\":3}</tool_call>")
                .is_empty()
        );
        // Empty name.
        assert!(
            find_tool_calls_in_text("<tool_call>{\"name\":\"\",\"arguments\":{}}</tool_call>")
                .is_empty()
        );
    }

    #[test]
    fn several_calls_are_returned_in_source_order() {
        let calls = find_tool_calls_in_text(
            "<tool_call>{\"name\":\"first\",\"arguments\":{}}</tool_call>\
             <tool_call>{\"name\":\"second\",\"arguments\":{}}</tool_call>",
        );
        assert_eq!(
            calls.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            ["first", "second"]
        );
    }

    /// A fenced block inside a tagged block must be counted once.
    #[test]
    fn nested_markup_is_not_double_counted() {
        let calls = find_tool_calls_in_text(
            "<tool_call>\n```json\n{\"name\":\"read_file\",\"arguments\":{}}\n```\n</tool_call>",
        );
        assert_eq!(calls.len(), 1);
    }

    #[test]
    fn stripping_leaves_the_surrounding_prose() {
        let text = "Reading it now.\n<tool_call>{\"name\":\"read_file\",\"arguments\":{}}</tool_call>\nDone.";
        let calls = find_tool_calls_in_text(text);
        assert_eq!(
            strip_recovered_spans(text, &calls),
            "Reading it now.\n\nDone."
        );
    }

    #[test]
    fn stripping_everything_yields_an_empty_string() {
        let text = "<tool_call>{\"name\":\"read_file\",\"arguments\":{}}</tool_call>";
        let calls = find_tool_calls_in_text(text);
        assert!(strip_recovered_spans(text, &calls).is_empty());
    }
}
