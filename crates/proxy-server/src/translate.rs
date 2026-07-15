use serde_json::{Map, Value};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TranslationError {
    #[error("missing field: {0}")]
    MissingField(&'static str),
    #[error("unsupported content block type: {0}")]
    UnsupportedBlockType(String),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Translate an Anthropic `/v1/messages` request body into an OpenAI
/// `/v1/chat/completions` request body.
///
/// Returns the OpenAI-compatible JSON value and the model name to send.
pub fn anthropic_to_openai(body: &Value) -> Result<Value, TranslationError> {
    let model = body
        .get("model")
        .and_then(|m| m.as_str())
        .ok_or(TranslationError::MissingField("model"))?;
    let max_tokens = body.get("max_tokens").and_then(|v| v.as_u64());
    let stream = body.get("stream").and_then(|v| v.as_bool()).unwrap_or(false);
    let _temperature = body.get("temperature").and_then(|v| v.as_f64());
    let _top_p = body.get("top_p").and_then(|v| v.as_f64());
    let _top_k = body.get("top_k").and_then(|v| v.as_u64());
    let stop_sequences = body.get("stop_sequences").and_then(|v| v.as_array());
    let has_thinking = body
        .get("thinking")
        .and_then(|t| t.get("type"))
        .and_then(|t| t.as_str())
        == Some("enabled");

    // Convert Anthropic messages to OpenAI messages.
    let messages = body
        .get("messages")
        .and_then(|v| v.as_array())
        .ok_or(TranslationError::MissingField("messages"))?;

    let mut openai_messages: Vec<Value> = Vec::new();

    // Convert system prompt to OpenAI system message.
    if let Some(system) = body.get("system") {
        let content = match system {
            Value::String(s) => s.clone(),
            Value::Array(blocks) => {
                let mut parts = Vec::new();
                for block in blocks {
                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                        parts.push(text.to_string());
                    }
                }
                parts.join("\n")
            }
            _ => String::new(),
        };
        if !content.is_empty() {
            let mut msg = Map::new();
            msg.insert("role".into(), Value::String("system".into()));
            msg.insert("content".into(), Value::String(content));
            openai_messages.push(Value::Object(msg));
        }
    }

    // Convert each Anthropic message.
    for msg in messages {
        let role = msg
            .get("role")
            .and_then(|r| r.as_str())
            .ok_or(TranslationError::MissingField("messages[].role"))?;

        let openai_role = match role {
            "assistant" => "assistant",
            "user" => "user",
            _ => return Err(TranslationError::UnsupportedBlockType(role.into())),
        };

        let content = msg.get("content");
        let openai_content = match content {
            Some(Value::String(text)) => Value::String(text.clone()),
            Some(Value::Array(blocks)) => {
                let parts: Vec<Value> = blocks
                    .iter()
                    .filter_map(|block| convert_content_block(block).ok())
                    .collect();
                Value::Array(parts)
            }
            _ => Value::Null,
        };

        let mut oa_msg = Map::new();
        oa_msg.insert("role".into(), Value::String(openai_role.into()));
        if !openai_content.is_null() {
            oa_msg.insert("content".into(), openai_content);
        }

        // Translate tool_use/tool_result content blocks (from assistant messages).
        if let Some(Value::Array(blocks)) = content {
            for block in blocks {
                match block.get("type").and_then(|t| t.as_str()) {
                    Some("tool_use") => {
                        if let Some(tool_calls) =
                            convert_tool_use_to_openai(block).transpose()
                        {
                            oa_msg.insert(
                                "tool_calls".into(),
                                Value::Array(vec![tool_calls?]),
                            );
                        }
                    }
                    Some("tool_result") => {
                        // OpenAI puts tool results in a separate "tool" role message.
                        if let Some(tool_msg) =
                            convert_tool_result_to_openai(block).transpose()
                        {
                            openai_messages.push(tool_msg?);
                        }
                    }
                    _ => {}
                }
            }
        }

        openai_messages.push(Value::Object(oa_msg));
    }

    // Build OpenAI request.
    let mut req = Map::new();
    req.insert("model".into(), Value::String(model.into()));
    req.insert(
        "messages".into(),
        Value::Array(openai_messages),
    );
    if let Some(mt) = max_tokens {
        req.insert("max_tokens".into(), Value::Number(mt.into()));
    }
    req.insert("stream".into(), Value::Bool(stream));
    if has_thinking {
        req.insert("reasoning_effort".into(), Value::String("medium".into()));
    }
    if let Some(stops) = stop_sequences {
        if !stops.is_empty() {
            req.insert("stop".into(), Value::Array(stops.clone()));
        }
    }

    Ok(Value::Object(req))
}

/// Convert an Anthropic content block to an OpenAI content part.
fn convert_content_block(block: &Value) -> Result<Value, TranslationError> {
    let block_type = block
        .get("type")
        .and_then(|t| t.as_str())
        .ok_or(TranslationError::MissingField("content_block.type"))?;
    match block_type {
        "text" => {
            let text = block
                .get("text")
                .and_then(|t| t.as_str())
                .unwrap_or("");
            let mut part = Map::new();
            part.insert("type".into(), Value::String("text".into()));
            part.insert("text".into(), Value::String(text.into()));
            Ok(Value::Object(part))
        }
        "image" => {
            let source = block
                .get("source")
                .ok_or(TranslationError::MissingField("image.source"))?;
            let media_type = source
                .get("media_type")
                .and_then(|m| m.as_str())
                .unwrap_or("image/png");
            let data = source
                .get("data")
                .and_then(|d| d.as_str())
                .unwrap_or("");
            let mut part = Map::new();
            part.insert("type".into(), Value::String("image_url".into()));
            let mut url = Map::new();
            url.insert(
                "url".into(),
                Value::String(format!("data:{};base64,{}", media_type, data)),
            );
            part.insert("image_url".into(), Value::Object(url));
            Ok(Value::Object(part))
        }
        "tool_use" | "tool_result" => Ok(Value::Null), // handled separately
        other => Err(TranslationError::UnsupportedBlockType(other.into())),
    }
}

/// Convert an Anthropic `tool_use` block to OpenAI `tool_calls` entry.
fn convert_tool_use_to_openai(block: &Value) -> Result<Option<Value>, TranslationError> {
    let name = block
        .get("name")
        .and_then(|n| n.as_str())
        .ok_or(TranslationError::MissingField("tool_use.name"))?;
    let id = block
        .get("id")
        .and_then(|i| i.as_str())
        .unwrap_or("");

    let mut tc = Map::new();
    tc.insert("id".into(), Value::String(id.into()));
    tc.insert("type".into(), Value::String("function".into()));
    let mut func = Map::new();
    func.insert("name".into(), Value::String(name.into()));
    let arguments = block.get("input").cloned().unwrap_or(Value::Object(Map::new()));
    func.insert(
        "arguments".into(),
        Value::String(arguments.to_string()),
    );
    tc.insert("function".into(), Value::Object(func));
    Ok(Some(Value::Object(tc)))
}

/// Convert an Anthropic `tool_result` block to an OpenAI tool role message.
fn convert_tool_result_to_openai(block: &Value) -> Result<Option<Value>, TranslationError> {
    let tool_use_id = block
        .get("tool_use_id")
        .and_then(|i| i.as_str())
        .ok_or(TranslationError::MissingField("tool_result.tool_use_id"))?;

    let content = match block.get("content") {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(parts)) => {
            let texts: Vec<&str> = parts
                .iter()
                .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                .collect();
            texts.join("\n")
        }
        _ => String::new(),
    };

    let mut msg = Map::new();
    msg.insert("role".into(), Value::String("tool".into()));
    msg.insert(
        "content".into(),
        Value::String(content),
    );
    msg.insert(
        "tool_call_id".into(),
        Value::String(tool_use_id.into()),
    );
    Ok(Some(Value::Object(msg)))
}

/// Translate an OpenAI `/v1/chat/completions` response (non-streaming) back
/// to Anthropic `/v1/messages` response format.
pub fn openai_to_anthropic(body: &Value) -> Result<Value, TranslationError> {
    let choice = body
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|c| c.first())
        .ok_or(TranslationError::MissingField("choices[0]"))?;

    let message = choice
        .get("message")
        .ok_or(TranslationError::MissingField("choices[0].message"))?;

    let role = message
        .get("role")
        .and_then(|r| r.as_str())
        .unwrap_or("assistant");

    let content = message.get("content").and_then(|c| c.as_str());

    let finish_reason = choice.get("finish_reason").and_then(|f| f.as_str());

    let usage = body.get("usage");

    // Build content blocks.
    let mut blocks: Vec<Value> = Vec::new();

    if let Some(text) = content {
        if !text.is_empty() {
            let mut tb = Map::new();
            tb.insert("type".into(), Value::String("text".into()));
            tb.insert("text".into(), Value::String(text.into()));
            blocks.push(Value::Object(tb));
        }
    }

    // Convert tool_calls to tool_use blocks.
    if let Some(tcs) = message.get("tool_calls").and_then(|c| c.as_array()) {
        for tc in tcs {
            let id = tc.get("id").and_then(|i| i.as_str()).unwrap_or("");
            let function = tc.get("function").ok_or(TranslationError::MissingField("tool_calls[].function"))?;
            let name = function.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let args_str = function.get("arguments").and_then(|a| a.as_str()).unwrap_or("{}");
            let args: Value = serde_json::from_str(args_str).unwrap_or(Value::Object(Map::new()));

            let mut tu = Map::new();
            tu.insert("type".into(), Value::String("tool_use".into()));
            tu.insert("id".into(), Value::String(id.into()));
            tu.insert("name".into(), Value::String(name.into()));
            tu.insert("input".into(), args);
            blocks.push(Value::Object(tu));
        }
    }

    // Build Anthropic response.
    let mut resp = Map::new();
    resp.insert("id".into(), Value::String(format!("msg_{:016x}", rand_u64())));
    resp.insert("type".into(), Value::String("message".into()));
    resp.insert("role".into(), Value::String(role.into()));
    resp.insert("content".into(), Value::Array(blocks));

    let stop_reason = match finish_reason {
        Some("stop") => "end_turn",
        Some("length") => "max_tokens",
        Some("tool_calls") => "tool_use",
        _ => "end_turn",
    };
    resp.insert("stop_reason".into(), Value::String(stop_reason.into()));
    resp.insert("stop_sequence".into(), Value::Null);

    if let Some(u) = usage {
        let input_tokens = u.get("prompt_tokens").and_then(|t| t.as_u64()).unwrap_or(0);
        let output_tokens = u.get("completion_tokens").and_then(|t| t.as_u64()).unwrap_or(0);
        let mut au = Map::new();
        au.insert("input_tokens".into(), Value::Number(input_tokens.into()));
        au.insert("output_tokens".into(), Value::Number(output_tokens.into()));
        resp.insert("usage".into(), Value::Object(au));
    }

    Ok(Value::Object(resp))
}

/// Translate an OpenAI SSE stream event line into the corresponding
/// Anthropic SSE event.
///
/// Returns `(event_name, data_json)`. Returns `None` for events that should
/// be skipped (e.g., internal OpenAI events).
pub fn translate_stream_event(
    event: &str,
    data: &str,
) -> Result<Option<(String, Value)>, TranslationError> {
    match event {
        "data" => {
            if data == "[DONE]" {
                return Ok(Some(("message_stop".into(), Value::Object(Map::new()))));
            }
            let chunk: Value = serde_json::from_str(data)?;
            translate_stream_chunk(&chunk)
        }
        _ => Ok(None),
    }
}

fn translate_stream_chunk(chunk: &Value) -> Result<Option<(String, Value)>, TranslationError> {
    let choices = chunk
        .get("choices")
        .and_then(|c| c.as_array())
        .ok_or(TranslationError::MissingField("choices"))?;

    if choices.is_empty() {
        return Ok(None);
    }

    let delta = &choices[0];
    let finish_reason = delta.get("finish_reason").and_then(|f| f.as_str());
    let index = delta.get("index").and_then(|i| i.as_u64()).unwrap_or(0);

    // Check for tool_calls in delta.
    let tool_calls = delta.get("delta").and_then(|d| d.get("tool_calls"));

    // Check for regular content.
    let content = delta
        .get("delta")
        .and_then(|d| d.get("content"))
        .and_then(|c| c.as_str());

    let has_reasoning = delta.get("delta").and_then(|d| d.get("reasoning_content"));

    // Build the appropriate Anthropic event.
    match (content, tool_calls, finish_reason) {
        // Start of content block.
        (Some(text), _, _) if text == "" && index == 0 => {
            let mut cb = Map::new();
            cb.insert("type".into(), Value::String("content_block_start".into()));
            cb.insert("index".into(), Value::Number(0.into()));
            let mut block = Map::new();
            block.insert("type".into(), Value::String("text".into()));
            block.insert("text".into(), Value::String("".into()));
            cb.insert("content_block".into(), Value::Object(block));
            Ok(Some(("content_block_start".into(), Value::Object(cb))))
        }
        // Content delta.
        (Some(text), _, _) if !text.is_empty() => {
            let mut cd = Map::new();
            cd.insert("type".into(), Value::String("content_block_delta".into()));
            cd.insert("index".into(), Value::Number(0.into()));
            let mut delta_map = Map::new();
            delta_map.insert("type".into(), Value::String("text_delta".into()));
            delta_map.insert("text".into(), Value::String(text.into()));
            cd.insert("delta".into(), Value::Object(delta_map));
            Ok(Some(("content_block_delta".into(), Value::Object(cd))))
        }
        // Tool calls.
        (_, Some(tcs), _) => {
            let tcs_arr = tcs.as_array().ok_or(TranslationError::MissingField("tool_calls"))?;
            let mut results = Vec::new();
            for tc in tcs_arr {
                let id = tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0);
                let func = tc.get("function");
                let name = func.and_then(|f| f.get("name")).and_then(|n| n.as_str());
                let args = func.and_then(|f| f.get("arguments")).and_then(|a| a.as_str());

                if let Some(name) = name {
                    // Start of tool use block.
                    let mut cbs = Map::new();
                    cbs.insert("type".into(), Value::String("content_block_start".into()));
                    cbs.insert("index".into(), Value::Number(id.into()));
                    let mut tb = Map::new();
                    tb.insert("type".into(), Value::String("tool_use".into()));
                    tb.insert("id".into(), Value::String(format!("toolu_{:016x}", rand_u64())));
                    tb.insert("name".into(), Value::String(name.into()));
                    let parsed_args: Value = args
                        .and_then(|a| serde_json::from_str(a).ok())
                        .unwrap_or(Value::Object(Map::new()));
                    tb.insert("input".into(), parsed_args);
                    cbs.insert("content_block".into(), Value::Object(tb));
                    results.push(("content_block_start".into(), Value::Object(cbs)));
                } else if let Some(args) = args {
                    if !args.is_empty() && args != "{}" {
                        let mut cbd = Map::new();
                        cbd.insert("type".into(), Value::String("content_block_delta".into()));
                        cbd.insert("index".into(), Value::Number(id.into()));
                        let mut dm = Map::new();
                        dm.insert("type".into(), Value::String("input_json_delta".into()));
                        dm.insert("partial_json".into(), Value::String(args.into()));
                        cbd.insert("delta".into(), Value::Object(dm));
                        results.push(("content_block_delta".into(), Value::Object(cbd)));
                    }
                }
            }
            // Return the last tool event; In practice Claude Code handles
            // multiple stream events, so we take the first found.
            Ok(results.into_iter().next())
        }
        // Thinking/reasoning content.
        (_, _, _) if has_reasoning.is_some() => {
            let reasoning = has_reasoning.and_then(|r| r.as_str()).unwrap_or("");
            if !reasoning.is_empty() {
                let mut cd = Map::new();
                cd.insert("type".into(), Value::String("content_block_delta".into()));
                cd.insert("index".into(), Value::Number(0.into()));
                let mut dm = Map::new();
                dm.insert("type".into(), Value::String("thinking_delta".into()));
                dm.insert("thinking".into(), Value::String(reasoning.into()));
                cd.insert("delta".into(), Value::Object(dm));
                Ok(Some(("content_block_delta".into(), Value::Object(cd))))
            } else {
                Ok(None)
            }
        }
        // Finish.
        (_, _, Some(reason)) => {
            let stop_reason = match reason {
                "stop" => "end_turn",
                "length" => "max_tokens",
                "tool_calls" => "tool_use",
                _ => "end_turn",
            };
            // Emit content_block_stop first.
            let mut cbstop = Map::new();
            cbstop.insert("type".into(), Value::String("content_block_stop".into()));
            cbstop.insert("index".into(), Value::Number(0.into()));
            let cbs_event = ("content_block_stop".into(), Value::Object(cbstop));

            // Then message_delta with usage.
            let mut md = Map::new();
            md.insert("type".into(), Value::String("message_delta".into()));
            md.insert("stop_reason".into(), Value::String(stop_reason.into()));
            md.insert("stop_sequence".into(), Value::Null);
            let mut delta_map = Map::new();
            delta_map.insert("stop_reason".into(), Value::String(stop_reason.into()));
            delta_map.insert("stop_sequence".into(), Value::Null);
            md.insert("delta".into(), Value::Object(delta_map));
            if let Some(usage) = chunk.get("usage") {
                let mut au = Map::new();
                au.insert(
                    "input_tokens".into(),
                    usage
                        .get("prompt_tokens")
                        .cloned()
                        .unwrap_or(Value::Number(0.into())),
                );
                au.insert(
                    "output_tokens".into(),
                    usage
                        .get("completion_tokens")
                        .cloned()
                        .unwrap_or(Value::Number(0.into())),
                );
                md.insert("usage".into(), Value::Object(au));
            }
            let md_event = ("message_delta".into(), Value::Object(md));

            // Return content_block_stop; caller will see stop_reason and
            // send message_stop as final.
            Ok(Some(cbs_event))
        }
        _ => Ok(None),
    }
}

/// Count tokens locally (simple estimation).
pub fn count_tokens_estimate(text: &str) -> u64 {
    // Rough estimate: ~4 chars per token for English text.
    (text.len() as f64 / 4.0).ceil() as u64
}

fn rand_u64() -> u64 {
    // Simple non-cryptographic random for message IDs.
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64;
    (nanos << 32) | (nanos.wrapping_mul(6364136223846793005) >> 32)
}
