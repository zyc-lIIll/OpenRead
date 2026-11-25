use serde_json::{json, Value};

use crate::agent::tool::source::pdf::PageImage;
use crate::agent::ModelMessage;

use super::Event;

pub fn request_body(
    model: &str,
    messages: &[ModelMessage],
    tools: &[Value],
    require_tool: bool,
) -> Value {
    let messages = messages.iter().map(|message| {
        let mut value = json!({"role":message.role,"content":message.content});
        if !message.tool_calls.is_empty() {
            value["tool_calls"] = Value::Array(message.tool_calls.iter().map(|call| json!({"id":call.id,"type":"function","function":{"name":call.name,"arguments":call.arguments}})).collect());
        }
        if let Some(id) = &message.tool_call_id { value["tool_call_id"] = json!(id); }
        value
    }).collect::<Vec<_>>();
    let mut body = json!({"model":model,"messages":messages,"stream":true});
    if !tools.is_empty() {
        body["tools"] = Value::Array(
            tools
                .iter()
                .map(|tool| json!({"type":"function","function":tool}))
                .collect(),
        );
        body["tool_choice"] = json!(if require_tool { "required" } else { "auto" });
    }
    body
}

pub fn initial_request_body(
    model: &str,
    instructions: &str,
    input: &str,
    images: &[PageImage],
) -> Value {
    json!({"model":model,"messages":[
        {"role":"system","content":instructions},
        {"role":"user","content":content(input, images)}
    ],"stream":true})
}

pub fn content(input: &str, images: &[PageImage]) -> Value {
    if images.is_empty() {
        return Value::String(input.into());
    }
    let mut content = vec![json!({"type":"text","text":input})];
    for image in images {
        content
            .push(json!({"type":"text","text":format!("【第 {} 页页面图像】",image.page_number)}));
        content
            .push(json!({"type":"image_url","image_url":{"url":image.data_url,"detail":"auto"}}));
    }
    Value::Array(content)
}

pub(super) fn decode(value: &Value) -> Result<Vec<Event>, String> {
    let mut events = Vec::new();
    if let Some(text) = value
        .pointer("/choices/0/delta/content")
        .and_then(Value::as_str)
    {
        events.push(Event::Text(text.to_string()));
    }
    if let Some(calls) = value
        .pointer("/choices/0/delta/tool_calls")
        .and_then(Value::as_array)
    {
        for call in calls {
            let Some(index) = call
                .get("index")
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
            else {
                continue;
            };
            events.push(Event::ToolCallDelta {
                index,
                id: call.get("id").and_then(Value::as_str).map(str::to_owned),
                name: call
                    .pointer("/function/name")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                arguments: call
                    .pointer("/function/arguments")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            });
        }
    }
    if let Some(error) = value.get("error") {
        return Err(error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("模型请求失败。")
            .to_string());
    }
    match value
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str)
    {
        Some("length") => events.push(Event::Truncated(None)),
        Some("content_filter") => return Err("模型输出被内容安全策略截断。".into()),
        _ => {}
    }
    Ok(events)
}
