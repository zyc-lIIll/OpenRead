use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadingAnchor {
    pub level: u8,
    pub title: String,
    pub source_range: SourceRange,
    pub section_range: SourceRange,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageAnchor {
    pub destination: String,
    pub title: String,
    pub alt: String,
    pub source_range: SourceRange,
    pub heading_path: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkdownAnchors {
    pub headings: Vec<HeadingAnchor>,
    pub images: Vec<ImageAnchor>,
}

#[derive(Default)]
struct HeadingBuilder {
    level: u8,
    title: String,
    source_range: SourceRange,
}

#[derive(Default)]
struct ImageBuilder {
    destination: String,
    title: String,
    alt: String,
    source_range: SourceRange,
}

/// Extracts heading hierarchy and image source anchors from Markdown text.
/// Ranges are zero-based, half-open UTF-8 byte offsets into the original text.
pub fn parse(markdown: &str) -> MarkdownAnchors {
    let mut headings = Vec::new();
    let mut images = Vec::new();
    let mut heading: Option<HeadingBuilder> = None;
    let mut image: Option<ImageBuilder> = None;

    for (event, range) in Parser::new_ext(markdown, Options::all()).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                heading = Some(HeadingBuilder {
                    level: heading_level(level),
                    title: String::new(),
                    source_range: SourceRange {
                        start: range.start,
                        end: range.end,
                    },
                });
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(mut current) = heading.take() {
                    current.title = current
                        .title
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ");
                    current.source_range.end = range.end.max(current.source_range.start);
                    let section_start = current.source_range.start;
                    headings.push(HeadingAnchor {
                        level: current.level,
                        title: current.title,
                        source_range: current.source_range,
                        section_range: SourceRange {
                            start: section_start,
                            end: markdown.len(),
                        },
                    });
                }
            }
            Event::Start(Tag::Image {
                dest_url, title, ..
            }) => {
                image = Some(ImageBuilder {
                    destination: dest_url.to_string(),
                    title: title.to_string(),
                    alt: String::new(),
                    source_range: SourceRange {
                        start: range.start,
                        end: range.end,
                    },
                });
            }
            Event::End(TagEnd::Image) => {
                if let Some(mut current) = image.take() {
                    current.source_range.end = range.end.max(current.source_range.start);
                    if let Some(current_heading) = heading.as_mut() {
                        current_heading.title.push_str(&current.alt);
                    }
                    images.push(ImageAnchor {
                        destination: current.destination,
                        title: current.title,
                        alt: current.alt,
                        source_range: current.source_range,
                        heading_path: Vec::new(),
                    });
                }
            }
            Event::Text(text)
            | Event::Code(text)
            | Event::InlineMath(text)
            | Event::DisplayMath(text) => {
                if let Some(current) = image.as_mut() {
                    current.alt.push_str(&text);
                } else if let Some(current) = heading.as_mut() {
                    current.title.push_str(&text);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(current) = image.as_mut() {
                    current.alt.push(' ');
                } else if let Some(current) = heading.as_mut() {
                    current.title.push(' ');
                }
            }
            _ => {}
        }
    }

    let section_ends: Vec<usize> = headings
        .iter()
        .enumerate()
        .map(|(index, current)| {
            headings[index + 1..]
                .iter()
                .find(|next| next.level <= current.level)
                .map_or(markdown.len(), |next| next.source_range.start)
        })
        .collect();
    for (heading, end) in headings.iter_mut().zip(section_ends) {
        heading.section_range.end = end;
    }

    // Image paths need complete section ranges, so they are attached after parsing.
    for image in &mut images {
        image.heading_path = headings
            .iter()
            .filter(|heading| {
                heading.source_range.start <= image.source_range.start
                    && image.source_range.start < heading.section_range.end
            })
            .map(|heading| heading.title.clone())
            .collect();
    }

    MarkdownAnchors { headings, images }
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}
