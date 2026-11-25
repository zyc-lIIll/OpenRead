use std::collections::HashMap;

use super::super::super::tool::SourceCitation;
use super::Formatter;

#[test]
fn strips_split_valid_citation_and_attaches_stable_page_entity() {
    let refs = HashMap::from([(
        "r1".into(),
        SourceCitation {
            id: "r1".into(),
            source_id: "source-1".into(),
            source_name: "Paper.pdf".into(),
            page_number: 4,
        },
    )]);
    let mut formatter = Formatter::default();
    assert_eq!(formatter.push("根据论文[[cit", &refs), "根据论文");
    assert_eq!(formatter.push("e:r1]", &refs), "");
    assert_eq!(formatter.push("]可知。", &refs), "可知。");
    assert_eq!(formatter.finish(), "");
    let annotation = formatter.annotations().next().unwrap();
    assert_eq!(annotation["type"], "annotation");
    assert_eq!(annotation["index"], 4);
    assert_eq!(annotation["source"]["data"]["pageNumber"], "4");
}

#[test]
fn invalid_citations_are_removed_and_plain_text_streams() {
    let mut formatter = Formatter::default();
    assert_eq!(
        formatter.push("普通文字[[cite:r404]]结束", &HashMap::new()),
        "普通文字结束"
    );
    assert_eq!(formatter.finish(), "");
    assert_eq!(formatter.annotations().count(), 0);
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
    let output = vec![serde_json::json!({"content":[{"annotations":[
        {"type":"url_citation","url":"https://example.com/paper","title":"Paper","start_index":7,"end_index":12},
        {"type":"url_citation","url":"javascript:alert(1)","title":"bad","start_index":8,"end_index":9}
    ]}]})];
    let annotations = super::external_annotations(&output, &Formatter::default(), 0);
    assert_eq!(annotations.len(), 1);
    assert_eq!(annotations[0]["source"]["url"], "https://example.com/paper");
    assert_eq!(annotations[0]["index"], 12);
}

#[test]
fn web_citation_offsets_are_adjusted_after_hidden_source_markers() {
    let refs = HashMap::from([(
        "r1".into(),
        SourceCitation {
            id: "r1".into(),
            source_id: "source-1".into(),
            source_name: "Paper.pdf".into(),
            page_number: 4,
        },
    )]);
    let mut formatter = Formatter::default();
    assert_eq!(
        formatter.push("Claim [[cite:r1]] and web source", &refs),
        "Claim  and web source"
    );
    let output = vec![serde_json::json!({"content":[{"annotations":[
        {"type":"url_citation","url":"https://example.com","title":"web","start_index":22,"end_index":32}
    ]}]})];
    let annotation = super::external_annotations(&output, &formatter, 0)
        .pop()
        .unwrap();
    assert_eq!(annotation["index"], 21);
}

#[test]
fn web_citation_offsets_include_prior_model_turns() {
    let mut formatter = Formatter::default();
    formatter.push("Earlier answer. ", &HashMap::new());
    let raw_offset = formatter.raw_chars();
    formatter.push("web result", &HashMap::new());
    let output = vec![serde_json::json!({"content":[{"annotations":[
        {"type":"url_citation","url":"https://example.com","title":"web","start_index":0,"end_index":10}
    ]}]})];
    let annotation = super::external_annotations(&output, &formatter, raw_offset)
        .pop()
        .unwrap();
    assert_eq!(annotation["index"], 26);
}

#[test]
fn web_citations_include_prior_output_messages_in_the_same_turn() {
    let mut formatter = Formatter::default();
    formatter.push("First. Second.", &HashMap::new());
    let output = vec![serde_json::json!({"content":[
        {"type":"output_text","text":"First.","annotations":[
            {"type":"url_citation","url":"https://example.com/one","title":"one","end_index":6}
        ]},
        {"type":"output_text","text":"Second.","annotations":[
            {"type":"url_citation","url":"https://example.com/two","title":"two","end_index":7}
        ]}
    ]})];
    let annotations = super::external_annotations(&output, &formatter, 0);
    assert_eq!(annotations[0]["index"], 6);
    assert_eq!(annotations[1]["index"], 13);
}
