use std::collections::HashSet;

use super::super::tool::source::pdf::{format_page_ranges, PageRange, Segment};
use super::{
    append_citations, completion_instructions, compression_input, images_omitted_input,
    request_fallback, segment_input, unreadable_final_input, Citation, CompletionContext,
    COMPRESSION_INSTRUCTIONS, FINAL_OVERVIEW_INSTRUCTIONS, FINAL_SEARCH_INSTRUCTIONS,
    INITIAL_INSTRUCTIONS, NO_PAGE_RANGE_INSTRUCTIONS, OVERVIEW_CHAR_LIMIT, PAGE_RANGE_INSTRUCTIONS,
    RESPONSE_CHAR_LIMIT, UNREADABLE_FINAL_INSTRUCTIONS,
};

#[test]
fn segment_requests_include_the_previous_overview_and_new_source() {
    let segment = Segment {
        page_range: PageRange { start: 3, end: 3 },
        text: "【第 3 页】新原文片段".into(),
        images: Vec::new(),
        image_failures: Vec::new(),
        has_text: true,
        last_page_complete: true,
    };
    let input = segment_input(
        "示例文档",
        &segment,
        8,
        4,
        None,
        true,
        "# 目前概览\n\n## 下一部分关注点\n留意作者如何定义术语。",
    );

    assert!(input.contains("Source 名称：示例文档"));
    assert!(!input.contains("已随文字提供页图"));
    assert!(input.contains("## 下一部分关注点\n留意作者如何定义术语。"));
    assert!(input.contains("本轮新增原文：\n【第 3 页】新原文片段"));
}

#[test]
fn search_intent_only_changes_the_shared_instructions() {
    let middle = completion_instructions(
        CompletionContext::SourceSegment {
            final_segment: false,
            can_recommend_pages: true,
        },
        true,
    );
    let final_segment = completion_instructions(
        CompletionContext::SourceSegment {
            final_segment: true,
            can_recommend_pages: false,
        },
        true,
    );
    let no_search = completion_instructions(
        CompletionContext::SourceSegment {
            final_segment: true,
            can_recommend_pages: false,
        },
        false,
    );
    let compression = completion_instructions(CompletionContext::Compression, false);
    let unreadable_final = completion_instructions(CompletionContext::UnreadableFinal, true);

    assert!(!INITIAL_INSTRUCTIONS.contains("整理完概览后"));
    assert!(!middle.contains(FINAL_OVERVIEW_INSTRUCTIONS));
    assert!(middle.contains(PAGE_RANGE_INSTRUCTIONS));
    assert!(!middle.contains(FINAL_SEARCH_INSTRUCTIONS));
    assert!(final_segment.contains(FINAL_OVERVIEW_INSTRUCTIONS));
    assert!(final_segment.ends_with(FINAL_SEARCH_INSTRUCTIONS));
    assert!(no_search.contains(FINAL_OVERVIEW_INSTRUCTIONS));
    assert!(!no_search.contains(FINAL_SEARCH_INSTRUCTIONS));
    assert!(no_search.contains("当前请求没有联网搜索工具"));
    assert!(compression.contains(COMPRESSION_INSTRUCTIONS));
    assert!(!compression.contains("每轮会收到"));
    assert!(!compression.contains(FINAL_OVERVIEW_INSTRUCTIONS));
    assert!(!compression.contains(FINAL_SEARCH_INSTRUCTIONS));
    assert!(compression.contains("当前请求没有联网搜索工具"));
    assert!(unreadable_final.contains(UNREADABLE_FINAL_INSTRUCTIONS));
    assert!(unreadable_final.ends_with(FINAL_SEARCH_INSTRUCTIONS));
}

#[test]
fn image_fallback_and_unreadable_final_input_are_explicit() {
    let images = [
        super::super::tool::source::pdf::PageImage {
            page_number: 4,
            data_url: "data:image/png;base64,iVBORw0KGgo=".into(),
        },
        super::super::tool::source::pdf::PageImage {
            page_number: 5,
            data_url: "data:image/png;base64,iVBORw0KGgo=".into(),
        },
    ];
    let input = images_omitted_input("本轮原文", &images);
    assert!(input.contains("未发送第 4-5 页的页面图像"));
    assert!(input.contains("请勿根据这些图像推断内容"));

    let segment = Segment {
        page_range: PageRange { start: 8, end: 9 },
        text: "（本页无可提取文字。）".into(),
        images: Vec::new(),
        image_failures: vec![8, 9],
        has_text: false,
        last_page_complete: true,
    };
    let input = unreadable_final_input("扫描文档", &segment, "# 已读部分");
    assert!(input.contains("第 8-9 页未能纳入概览"));
    assert!(input.contains("不可据此推测页面内容"));
}

#[test]
fn page_range_guidance_is_enabled_only_at_page_boundaries() {
    let unavailable = completion_instructions(
        CompletionContext::SourceSegment {
            final_segment: false,
            can_recommend_pages: false,
        },
        false,
    );
    assert!(unavailable.contains(NO_PAGE_RANGE_INSTRUCTIONS));
    assert!(!unavailable.contains(PAGE_RANGE_INSTRUCTIONS));
}

#[test]
fn overview_compression_preserves_the_right_focus_without_referencing_raw_source() {
    let final_input = compression_input("# 概览\n\n## 后续精读关注点\n回看作者的研究方法。", true);
    let middle_input = compression_input("# 概览\n\n## 下一部分关注点\n留意术语定义。", false);

    assert!(final_input.contains("保留已有的“后续精读关注点”"));
    assert!(!final_input.contains("本轮原文"));
    assert!(middle_input.contains("保留已有的“下一部分关注点”"));
}

#[test]
fn citations_are_deduplicated_by_url() {
    let mut citations = Vec::new();
    let mut seen = HashSet::new();
    append_citations(
        &mut citations,
        &mut seen,
        vec![
            Citation {
                title: "First".into(),
                url: "https://example.com/a".into(),
            },
            Citation {
                title: "Duplicate".into(),
                url: "https://example.com/a".into(),
            },
        ],
    );
    assert_eq!(
        citations,
        vec![Citation {
            title: "First".into(),
            url: "https://example.com/a".into()
        }]
    );
}

#[test]
fn output_and_page_range_limits_are_stable() {
    assert_eq!(OVERVIEW_CHAR_LIMIT, 6_000);
    assert_eq!(RESPONSE_CHAR_LIMIT, 32_000);
    assert_eq!(format_page_ranges(&[1, 2, 3, 6, 8, 9]), "1-3、6、8-9");
}

#[test]
fn image_rejections_retry_without_images_but_other_errors_do_not() {
    assert_eq!(request_fallback(true, true), Some(false));
    assert_eq!(request_fallback(true, false), None);
    assert_eq!(request_fallback(false, true), None);
}
