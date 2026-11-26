use std::collections::HashMap;

use crate::agent::{model::WebCitation, tool::SourceCitation};

const PREFIX: &str = "[[cite:";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CitationTarget {
    Source(SourceCitation),
    Url { title: String, url: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Citation {
    pub target: CitationTarget,
    pub index: usize,
}

#[derive(Default)]
pub(crate) struct Formatter {
    pending: String,
    pending_start: usize,
    raw_chars: usize,
    visible_chars: usize,
    citations: Vec<Citation>,
    removed_spans: Vec<(usize, usize)>,
}

impl Formatter {
    pub fn push(&mut self, delta: &str, citations: &HashMap<String, CitationTarget>) -> String {
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
                    if partial_citation_id(partial) {
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
                if !valid_citation_id(id) {
                    let literal_end = start + PREFIX.len() + end + 2;
                    visible.push_str(&self.pending[start..literal_end]);
                    self.pending_start += self.pending[..literal_end].chars().count();
                    self.pending = self.pending[literal_end..].to_owned();
                    continue;
                }
                let marker_len = PREFIX.chars().count() + id.chars().count() + 2;
                self.removed_spans.push((marker_start, marker_len));
                if let Some(citation) = citations.get(id) {
                    self.citations.push(Citation {
                        target: citation.clone(),
                        index: self.visible_chars + visible.chars().count(),
                    });
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

    pub fn take_citations(&mut self) -> Vec<Citation> {
        std::mem::take(&mut self.citations)
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

fn partial_citation_id(value: &str) -> bool {
    let value = value.strip_suffix(']').unwrap_or(value);
    if value.is_empty() {
        return true;
    }
    let digits = value.strip_prefix('r').or_else(|| value.strip_prefix('w'));
    digits.is_some_and(|digits| {
        digits.len() <= 10 && digits.bytes().all(|byte| byte.is_ascii_digit())
    })
}

fn valid_citation_id(value: &str) -> bool {
    let digits = value.strip_prefix('r').or_else(|| value.strip_prefix('w'));
    digits.is_some_and(|digits| {
        !digits.is_empty() && digits.len() <= 10 && digits.bytes().all(|byte| byte.is_ascii_digit())
    })
}

pub fn external_citations(
    citations: &[WebCitation],
    formatter: &Formatter,
    raw_offset: usize,
) -> Vec<Citation> {
    citations
        .iter()
        .filter_map(|citation| {
            let parsed = url::Url::parse(&citation.url).ok()?;
            if !matches!(parsed.scheme(), "http" | "https") || parsed.host().is_none() {
                return None;
            }
            Some(Citation {
                target: CitationTarget::Url {
                    title: citation.title.clone(),
                    url: parsed.to_string(),
                },
                index: formatter.adjusted_index(raw_offset + citation.index),
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "../../../tests/agent_citation.rs"]
mod tests;
