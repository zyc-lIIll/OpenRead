use std::collections::HashSet;

pub const SEGMENT_CHAR_LIMIT: usize = 12_000;
pub const MAX_PAGE_IMAGES: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageRange {
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageImage {
    pub page_number: u32,
    pub data_url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PagePart {
    pub page_number: u32,
    pub text: String,
    pub last_part: bool,
    pub has_text: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SegmentBuilder {
    text: String,
    pages: Vec<u32>,
    images: Vec<PageImage>,
    image_failures: Vec<u32>,
    has_text: bool,
    last_page_complete: bool,
}

#[derive(Clone, Debug)]
pub struct Segment {
    pub page_range: PageRange,
    pub text: String,
    pub images: Vec<PageImage>,
    pub image_failures: Vec<u32>,
    pub has_text: bool,
    pub last_page_complete: bool,
}

impl SegmentBuilder {
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    pub fn would_exceed(&self, part: &PagePart) -> bool {
        if self.is_empty() {
            return false;
        }
        let adds_page = self.pages.last().copied() != Some(part.page_number);
        self.text.chars().count() + part.text.chars().count() + 2 > SEGMENT_CHAR_LIMIT
            || (adds_page && self.pages.len() >= MAX_PAGE_IMAGES)
    }

    pub fn push(&mut self, part: PagePart, image_data_url: Option<&str>, image_failed: bool) {
        if self.pages.last().copied() != Some(part.page_number) {
            self.pages.push(part.page_number);
            if image_failed {
                self.image_failures.push(part.page_number);
            }
            if let Some(data_url) = image_data_url {
                self.images.push(PageImage {
                    page_number: part.page_number,
                    data_url: data_url.to_owned(),
                });
            }
        }
        if !self.text.is_empty() {
            self.text.push_str("\n\n");
        }
        self.text.push_str(&part.text);
        self.has_text |= part.has_text;
        self.last_page_complete = part.last_part;
    }

    pub fn finish(&mut self) -> Option<Segment> {
        if self.is_empty() {
            return None;
        }
        let pages = std::mem::take(&mut self.pages);
        let start = pages[0];
        let end = *pages.last().unwrap_or(&start);
        Some(Segment {
            page_range: PageRange { start, end },
            text: std::mem::take(&mut self.text),
            images: std::mem::take(&mut self.images),
            image_failures: std::mem::take(&mut self.image_failures),
            has_text: std::mem::replace(&mut self.has_text, false),
            last_page_complete: std::mem::replace(&mut self.last_page_complete, false),
        })
    }
}

pub fn page_parts(page_number: u32, text: &str) -> Vec<PagePart> {
    let has_text = !text.trim().is_empty();
    let body = if has_text {
        text.to_owned()
    } else {
        "（本页无可提取文字。）".into()
    };
    let page_prefix = format!("【第 {page_number} 页】\n");
    let continuation_prefix = format!("【第 {page_number} 页（续）】\n");
    let body_limit = SEGMENT_CHAR_LIMIT.saturating_sub(continuation_prefix.chars().count());
    let parts = split_text(&body, body_limit);
    let parts_len = parts.len();
    parts
        .into_iter()
        .enumerate()
        .map(|(index, body)| PagePart {
            page_number,
            text: format!(
                "{}{}",
                if index == 0 {
                    page_prefix.as_str()
                } else {
                    continuation_prefix.as_str()
                },
                body
            ),
            last_part: index + 1 == parts_len,
            has_text,
        })
        .collect()
}

pub fn extract_page_range(markdown: &str) -> (String, Option<PageRange>, bool) {
    const MARKER: &str = "<!-- openread-next-pages:";
    let mut remaining = markdown;
    let mut cleaned = String::with_capacity(markdown.len());
    let mut result = None;
    let mut found = false;
    let mut invalid = false;
    while let Some(marker_start) = remaining.find(MARKER) {
        found = true;
        cleaned.push_str(&remaining[..marker_start]);
        let value_start = marker_start + MARKER.len();
        let Some(marker_end) = remaining[value_start..].find("-->") else {
            return (cleaned.trim().to_owned(), None, true);
        };
        let parsed = parse_range(&remaining[value_start..value_start + marker_end]);
        if result.is_some() || parsed.is_none() {
            invalid = true;
        } else {
            result = parsed;
        }
        remaining = &remaining[value_start + marker_end + 3..];
    }
    cleaned.push_str(remaining);
    (
        cleaned.trim().to_owned(),
        if invalid { None } else { result },
        found,
    )
}

pub fn parse_range(value: &str) -> Option<PageRange> {
    let (start, end) = value.trim().split_once('-')?;
    let start = start.trim().parse::<u32>().ok()?;
    let end = end.trim().parse::<u32>().ok()?;
    (start > 0 && end >= start).then_some(PageRange { start, end })
}

pub fn valid_recommendation(range: PageRange, first_unread_page: u32, total_pages: u32) -> bool {
    range.start == first_unread_page
        && range.end <= total_pages
        && range.end - range.start < MAX_PAGE_IMAGES as u32
}

pub fn format_page_ranges(pages: &[u32]) -> String {
    let mut pages = pages.to_vec();
    pages.sort_unstable();
    pages.dedup();
    if pages.is_empty() {
        return String::new();
    }
    let mut ranges = Vec::new();
    let mut start = pages[0];
    let mut end = start;
    for page in pages.into_iter().skip(1) {
        if page == end + 1 {
            end = page;
        } else {
            ranges.push(format_range(start, end));
            start = page;
            end = page;
        }
    }
    ranges.push(format_range(start, end));
    ranges.join("、")
}

pub fn deduplicate_pages(pages: impl IntoIterator<Item = u32>) -> Vec<u32> {
    let mut seen = HashSet::new();
    pages
        .into_iter()
        .filter(|page| seen.insert(*page))
        .collect()
}

fn format_range(start: u32, end: u32) -> String {
    if start == end {
        start.to_string()
    } else {
        format!("{start}-{end}")
    }
}

fn split_text(text: &str, limit: usize) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut parts = Vec::new();
    let mut remaining = text;
    while remaining.chars().count() > limit {
        let mut split_at = remaining
            .char_indices()
            .nth(limit)
            .map(|(index, _)| index)
            .unwrap_or(remaining.len());
        if let Some(boundary) = remaining[..split_at].rfind('\n') {
            if remaining[..boundary].chars().count() >= limit / 2 {
                split_at = boundary + 1;
            }
        }
        parts.push(remaining[..split_at].to_owned());
        remaining = &remaining[split_at..];
    }
    if !remaining.is_empty() {
        parts.push(remaining.to_owned());
    }
    parts
}

#[cfg(test)]
#[path = "../../../../tests/agent_source_pdf.rs"]
mod tests;
