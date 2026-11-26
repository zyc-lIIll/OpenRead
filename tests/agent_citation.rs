use std::collections::HashMap;

use super::{external_citations, CitationTarget, Formatter};
use crate::agent::{model::WebCitation, tool::SourceCitation};

#[test]
fn strips_split_valid_citation_and_tracks_stable_page_location() {
    let refs = HashMap::from([(
        "r1".into(),
        CitationTarget::Source(SourceCitation {
            id: "r1".into(),
            source_id: "source-1".into(),
            source_name: "Paper.pdf".into(),
            page_number: 4,
        }),
    )]);
    let mut formatter = Formatter::default();
    assert_eq!(formatter.push("根据论文[[cit", &refs), "根据论文");
    assert_eq!(formatter.push("e:r1]", &refs), "");
    assert_eq!(formatter.push("]可知。", &refs), "可知。");
    assert_eq!(formatter.finish(), "");
    let citations = formatter.take_citations();
    let [citation] = citations.as_slice() else {
        panic!("expected one PDF citation");
    };
    assert_eq!(citation.index, 4);
    assert!(matches!(
        &citation.target,
        CitationTarget::Source(source) if source.page_number == 4 && source.source_id == "source-1"
    ));
}

#[test]
fn invalid_citations_are_removed_and_plain_text_streams() {
    let mut formatter = Formatter::default();
    assert_eq!(
        formatter.push("普通文字[[cite:r404]]结束", &HashMap::new()),
        "普通文字结束"
    );
    assert_eq!(formatter.finish(), "");
    assert!(formatter.take_citations().is_empty());
}

#[test]
fn streamed_web_citation_markers_map_to_validated_urls() {
    let refs = HashMap::from([(
        "w1".into(),
        CitationTarget::Url {
            title: "OpenRead source".into(),
            url: "https://example.com/source".into(),
        },
    )]);
    let mut formatter = Formatter::default();
    assert_eq!(formatter.push("Claim [[cite:w", &refs), "Claim ");
    assert_eq!(formatter.push("1]].", &refs), ".");
    let citations = formatter.take_citations();
    assert_eq!(citations.len(), 1);
    assert_eq!(citations[0].index, 6);
    assert!(matches!(
        &citations[0].target,
        CitationTarget::Url { url, .. } if url == "https://example.com/source"
    ));
}

#[test]
fn citation_prefix_split_before_its_id_remains_buffered() {
    let refs = HashMap::from([
        (
            "r1".into(),
            CitationTarget::Source(SourceCitation {
                id: "r1".into(),
                source_id: "source-1".into(),
                source_name: "Paper.pdf".into(),
                page_number: 1,
            }),
        ),
        (
            "w1".into(),
            CitationTarget::Url {
                title: "Source".into(),
                url: "https://example.com".into(),
            },
        ),
    ]);
    let mut formatter = Formatter::default();
    assert_eq!(formatter.push("A[[cite:", &refs), "A");
    assert_eq!(formatter.push("r1]] B[[cite:", &refs), " B");
    assert_eq!(formatter.push("w1]]", &refs), "");
    let citations = formatter.take_citations();
    assert_eq!(
        citations.iter().map(|item| item.index).collect::<Vec<_>>(),
        [1, 3]
    );
}

#[test]
fn malformed_or_unclosed_markers_do_not_consume_the_rest_of_the_answer() {
    let mut malformed = Formatter::default();
    assert_eq!(
        malformed.push("Keep [[cite:r1x and this text", &HashMap::new()),
        "Keep [[cite:r1x and this text"
    );
    assert_eq!(malformed.finish(), "");

    let mut unclosed = Formatter::default();
    assert_eq!(unclosed.push("Keep [[cite:r1", &HashMap::new()), "Keep ");
    assert_eq!(unclosed.finish(), "[[cite:r1");
}

#[test]
fn only_http_and_https_search_citations_are_attached() {
    let formatter = Formatter::default();
    let citations = vec![
        WebCitation {
            title: "Paper".into(),
            url: "https://example.com/paper".into(),
            index: 12,
        },
        WebCitation {
            title: "bad".into(),
            url: "javascript:alert(1)".into(),
            index: 9,
        },
    ];
    let citations = external_citations(&citations, &formatter, 0);
    assert_eq!(citations.len(), 1);
    assert_eq!(citations[0].index, 12);
    assert!(matches!(
        &citations[0].target,
        CitationTarget::Url { url, .. } if url == "https://example.com/paper"
    ));
}

#[test]
fn web_citation_offsets_skip_hidden_source_markers() {
    let refs = HashMap::from([(
        "r1".into(),
        CitationTarget::Source(SourceCitation {
            id: "r1".into(),
            source_id: "source-1".into(),
            source_name: "Paper.pdf".into(),
            page_number: 4,
        }),
    )]);
    let mut formatter = Formatter::default();
    assert_eq!(
        formatter.push("Claim [[cite:r1]] and web source", &refs),
        "Claim  and web source"
    );
    let citations = vec![WebCitation {
        title: "web".into(),
        url: "https://example.com".into(),
        index: 32,
    }];
    let citation = external_citations(&citations, &formatter, 0).pop().unwrap();
    assert_eq!(citation.index, 21);
}

#[test]
fn web_citation_offsets_include_prior_turns_in_the_same_segment() {
    let mut formatter = Formatter::default();
    formatter.push("Earlier answer. ", &HashMap::new());
    let raw_offset = formatter.raw_chars();
    formatter.push("web result", &HashMap::new());
    let citations = vec![WebCitation {
        title: "web".into(),
        url: "https://example.com".into(),
        index: 10,
    }];
    let citation = external_citations(&citations, &formatter, raw_offset)
        .pop()
        .unwrap();
    assert_eq!(citation.index, 26);
}

#[test]
fn web_citation_offsets_are_local_to_each_formal_segment() {
    let mut formatter = Formatter::default();
    formatter.push("First. Second.", &HashMap::new());
    let citations = vec![
        WebCitation {
            title: "one".into(),
            url: "https://example.com/one".into(),
            index: 6,
        },
        WebCitation {
            title: "two".into(),
            url: "https://example.com/two".into(),
            index: 13,
        },
    ];
    let citations = external_citations(&citations, &formatter, 0);
    assert_eq!(citations[0].index, 6);
    assert_eq!(citations[1].index, 13);
}
