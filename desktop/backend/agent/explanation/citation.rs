use std::collections::HashMap;

use serde_json::{json, Value};

use crate::agent::{model::WebCitation, tool::SourceCitation};

const PREFIX: &str = "[[cite:";

#[derive(Default)]
pub struct Formatter {
    pending: String,
    pending_start: usize,
    raw_chars: usize,
    visible_chars: usize,
    annotations: Vec<Value>,
    removed_spans: Vec<(usize, usize)>,
}

impl Formatter {
    pub fn push(&mut self, delta: &str, citations: &HashMap<String, SourceCitation>) -> String {
        if self.pending.is_empty() {
            self.pending_start = self.raw_chars;
        }
        self.pending.push_str(delta);
        let mut visible = String::new();
        loop {
            if let Some(start) = self.pending.find(PREFIX) {
                visible.push_str(&self.pending[..start]);
                let marker_start = self.pending_start + self.pending[..start].chars().count();
                let after = &self.pending[start + PREFIX.len()..];
                let Some(end) = after.find("]]") else {
                    let partial = after.strip_suffix(']').unwrap_or(after);
                    let partial_id = partial.strip_prefix('r').unwrap_or(partial);
                    if partial_id.len() <= 10
                        && partial_id.bytes().all(|byte| byte.is_ascii_digit())
                    {
                        self.pending = self.pending[start..].to_owned();
                        self.pending_start = marker_start;
                        break;
                    }
                    let literal_end = start + PREFIX.len();
                    visible.push_str(&self.pending[start..literal_end]);
                    self.pending_start += self.pending[..literal_end].chars().count();
                    self.pending = self.pending[literal_end..].to_owned();
                    continue;
                };
                let id = &after[..end];
                if !id.strip_prefix('r').is_some_and(|digits| {
                    !digits.is_empty()
                        && digits.len() <= 10
                        && digits.bytes().all(|byte| byte.is_ascii_digit())
                }) {
                    let literal_end = start + PREFIX.len() + end + 2;
                    visible.push_str(&self.pending[start..literal_end]);
                    self.pending_start += self.pending[..literal_end].chars().count();
                    self.pending = self.pending[literal_end..].to_owned();
                    continue;
                }
                let marker_len = PREFIX.chars().count() + id.chars().count() + 2;
                self.removed_spans.push((marker_start, marker_len));
                if let Some(citation) = citations.get(id) {
                    let index = self.visible_chars + visible.chars().count();
                    self.annotations.push(annotation(citation, index));
                }
                self.pending = after[end + 2..].to_owned();
                self.pending_start = marker_start + marker_len;
                continue;
            }
            let keep = (1..=PREFIX.len())
                .rev()
                .find(|size| self.pending.ends_with(&PREFIX[..*size]))
                .unwrap_or(0);
            let emit = self.pending.len().saturating_sub(keep);
            visible.push_str(&self.pending[..emit]);
            self.pending_start += self.pending[..emit].chars().count();
            self.pending = self.pending[emit..].to_owned();
            break;
        }
        self.raw_chars += delta.chars().count();
        self.visible_chars += visible.chars().count();
        visible
    }

    pub fn finish(&mut self) -> String {
        let tail = std::mem::take(&mut self.pending);
        self.visible_chars += tail.chars().count();
        tail
    }

    pub fn annotations(&self) -> impl Iterator<Item = Value> + '_ {
        self.annotations.iter().cloned()
    }

    pub fn raw_chars(&self) -> usize {
        self.raw_chars
    }

    fn adjusted_index(&self, index: usize) -> usize {
        let removed = self
            .removed_spans
            .iter()
            .filter(|(start, length)| *start + *length <= index)
            .map(|(_, length)| *length)
            .sum::<usize>();
        index.saturating_sub(removed)
    }
}

pub fn external_annotations(
    citations: &[WebCitation],
    formatter: &Formatter,
    raw_offset: usize,
) -> Vec<Value> {
    citations
        .iter()
        .filter_map(|citation| {
            let parsed = url::Url::parse(&citation.url).ok()?;
            if !matches!(parsed.scheme(), "http" | "https") || parsed.host().is_none() {
                return None;
            }
            Some(json!({
                "type":"annotation",
                "source":{"type":"url","url":parsed.as_str(),"title":citation.title},
                "index":formatter.adjusted_index(raw_offset + citation.index)
            }))
        })
        .collect()
}

fn annotation(citation: &SourceCitation, index: usize) -> Value {
    json!({
        "type":"annotation",
        "source":{
            "type":"entity",
            "id":format!("{}:{}",citation.source_id,citation.page_number),
            "title":citation.source_name,
            "description":format!("PDF 第 {} 页",citation.page_number),
            "label":"原文位置",
            "inline_label":format!("p.{}",citation.page_number),
            "interactive":true,
            "data":{"sourceId":citation.source_id,"pageNumber":citation.page_number.to_string()}
        },
        "index":index
    })
}

#[cfg(test)]
#[path = "../../../../tests/agent_citation.rs"]
mod tests;
