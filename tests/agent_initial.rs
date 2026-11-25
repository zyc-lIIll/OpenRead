use std::collections::HashSet;

use super::super::sse_frame_end;
use serde_json::json;
use tauri::ipc::Channel;

use super::super::source::pdf::{format_page_ranges, PageRange, Segment};
use super::super::ModelRoute;
use super::{
    chat_completion_content, collect_citations, completion_instructions, compression_input,
    ensure_frame_buffer_bounded, ensure_sse_buffer_bounded, error_rejects_images, handle_frame,
    images_omitted_input, request_fallback, responses_input_content, segment_input,
    unreadable_final_input, Citation, CompletionContext, COMPRESSION_INSTRUCTIONS,
    FINAL_OVERVIEW_INSTRUCTIONS, FINAL_SEARCH_INSTRUCTIONS, INITIAL_INSTRUCTIONS,
    NO_PAGE_RANGE_INSTRUCTIONS, OVERVIEW_CHAR_LIMIT, PAGE_RANGE_INSTRUCTIONS, RESPONSE_CHAR_LIMIT,
    SSE_BUFFER_BYTE_LIMIT, SSE_FRAME_BYTE_LIMIT, UNREADABLE_FINAL_INSTRUCTIONS,
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
fn final_search_instruction_is_added_only_when_search_is_available_for_the_final_segment() {
    let chatgpt = ModelRoute::Chatgpt {
        model_id: "model".into(),
        registration_id: "registration".into(),
    };
    let api = ModelRoute::Api {
        provider_id: "provider".into(),
        model_id: "model".into(),
    };

    let middle = completion_instructions(
        &chatgpt,
        CompletionContext::SourceSegment {
            final_segment: false,
            can_recommend_pages: true,
        },
        true,
    );
    let final_segment = completion_instructions(
        &chatgpt,
        CompletionContext::SourceSegment {
            final_segment: true,
            can_recommend_pages: false,
        },
        true,
    );
    let final_retry = completion_instructions(
        &chatgpt,
        CompletionContext::SourceSegment {
            final_segment: true,
            can_recommend_pages: false,
        },
        false,
    );
    let api_final = completion_instructions(
        &api,
        CompletionContext::SourceSegment {
            final_segment: true,
            can_recommend_pages: false,
        },
        true,
    );
    let compression = completion_instructions(&chatgpt, CompletionContext::Compression, false);
    let unreadable_final =
        completion_instructions(&chatgpt, CompletionContext::UnreadableFinal, true);

    assert!(!INITIAL_INSTRUCTIONS.contains("整理完概览后"));
    assert!(!middle.contains(FINAL_OVERVIEW_INSTRUCTIONS));
    assert!(middle.contains(PAGE_RANGE_INSTRUCTIONS));
    assert!(!middle.contains(FINAL_SEARCH_INSTRUCTIONS));
    assert!(final_segment.contains(FINAL_OVERVIEW_INSTRUCTIONS));
    assert!(final_segment.ends_with(FINAL_SEARCH_INSTRUCTIONS));
    assert!(final_retry.contains(FINAL_OVERVIEW_INSTRUCTIONS));
    assert!(!final_retry.contains(FINAL_SEARCH_INSTRUCTIONS));
    assert!(final_retry.contains("当前请求没有联网搜索工具"));
    assert!(compression.contains(COMPRESSION_INSTRUCTIONS));
    assert!(!compression.contains("每轮会收到"));
    assert!(!compression.contains(FINAL_OVERVIEW_INSTRUCTIONS));
    assert!(!compression.contains(FINAL_SEARCH_INSTRUCTIONS));
    assert!(compression.contains("当前请求没有联网搜索工具"));
    assert!(api_final.contains(FINAL_OVERVIEW_INSTRUCTIONS));
    assert!(!api_final.contains(FINAL_SEARCH_INSTRUCTIONS));
    assert!(api_final.contains("当前 API 路由没有联网工具"));
    assert!(unreadable_final.contains(UNREADABLE_FINAL_INSTRUCTIONS));
    assert!(unreadable_final.ends_with(FINAL_SEARCH_INSTRUCTIONS));
}

#[test]
fn image_fallback_and_unreadable_final_input_are_explicit() {
    let images = [
        super::super::source::pdf::PageImage {
            page_number: 4,
            data_url: "data:image/png;base64,iVBORw0KGgo=".into(),
        },
        super::super::source::pdf::PageImage {
            page_number: 5,
            data_url: "data:image/png;base64,iVBORw0KGgo=".into(),
        },
    ];
    let input = images_omitted_input("本轮原文", &images);
    assert!(input.contains("未发送第 4-5 页的页面图像"));
    assert!(input.contains("请勿根据这些图像推断内容"));

    assert!(error_rejects_images(
        r#"{"error":{"message":"model does not support image inputs"}}"#
    ));
    assert!(error_rejects_images("Request entity too large"));
    assert!(!error_rejects_images(
        r#"{"error":{"message":"model was not found"}}"#
    ));
    assert!(!error_rejects_images(
        r#"{"error":{"code":"model_not_found","message":"model gpt-4-vision-preview was not found"}}"#
    ));
    assert!(error_rejects_images(
        r#"{"error":{"message":"input image was not found"}}"#
    ));

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
    let route = ModelRoute::Api {
        provider_id: "provider".into(),
        model_id: "model".into(),
    };
    let unavailable = completion_instructions(
        &route,
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
fn incomplete_sse_frames_have_a_fixed_byte_limit() {
    assert!(ensure_frame_buffer_bounded(&vec![0; SSE_FRAME_BYTE_LIMIT]).is_ok());
    assert!(ensure_frame_buffer_bounded(&vec![0; SSE_FRAME_BYTE_LIMIT + 1]).is_err());
    assert!(ensure_sse_buffer_bounded(&vec![0; SSE_BUFFER_BYTE_LIMIT]).is_ok());
    assert!(ensure_sse_buffer_bounded(&vec![0; SSE_BUFFER_BYTE_LIMIT + 1]).is_err());

    let mut complete = vec![b'x'; SSE_FRAME_BYTE_LIMIT + 1];
    complete.extend_from_slice(b"\n\n");
    let (frame_end, _) = sse_frame_end(&complete).unwrap();
    assert!(ensure_frame_buffer_bounded(&complete[..frame_end]).is_err());
}

#[test]
fn citations_are_deduplicated_and_reject_non_http_urls() {
    let value = json!({
        "output": [{
            "content": [{
                "annotations": [
                    {"type":"url_citation","title":"First","url":"https://example.com/a"},
                    {"type":"url_citation","title":"Duplicate","url":"https://example.com/a"},
                    {"type":"url_citation","title":"Local","url":"file:///tmp/a"}
                ]
            }]
        }]
    });
    let mut citations = Vec::new();
    let mut seen = HashSet::new();
    collect_citations(&value, &mut citations, &mut seen);
    assert_eq!(
        citations,
        vec![Citation {
            title: "First".into(),
            url: "https://example.com/a".into(),
        }]
    );
}

#[test]
fn response_completion_collects_text_and_url_citations() {
    let mut text = String::new();
    let mut citations = Vec::new();
    let mut seen = HashSet::new();
    let progress = Channel::new(|_| Ok(()));
    let delta = r#"data: {"type":"response.output_text.delta","delta":"概览"}"#.as_bytes();
    assert!(!handle_frame(
        delta,
        true,
        &mut text,
        &mut citations,
        &mut seen,
        &progress,
        1,
        1
    )
    .unwrap());
    let completed = br#"data: {"type":"response.completed","response":{"output":[{"type":"message","content":[{"annotations":[{"type":"url_citation","title":"Source","url":"https://example.com"}]}]}]}}"#;
    assert!(handle_frame(
        completed,
        true,
        &mut text,
        &mut citations,
        &mut seen,
        &progress,
        1,
        1
    )
    .unwrap());
    assert_eq!(text, "概览");
    assert_eq!(citations[0].url, "https://example.com/");
}

#[test]
fn output_and_page_range_limits_are_stable() {
    assert_eq!(OVERVIEW_CHAR_LIMIT, 6_000);
    assert_eq!(RESPONSE_CHAR_LIMIT, 32_000);
    assert_eq!(format_page_ranges(&[1, 2, 3, 6, 8, 9]), "1-3、6、8-9");
}

#[test]
fn request_payloads_pair_text_with_each_page_image() {
    let images = [
        super::super::source::pdf::PageImage {
            page_number: 2,
            data_url: "data:image/png;base64,iVBORw0KGgo=".into(),
        },
        super::super::source::pdf::PageImage {
            page_number: 3,
            data_url: "data:image/png;base64,iVBORw0KGgo=".into(),
        },
    ];

    let chat = chat_completion_content("文字上下文", &images);
    assert_eq!(chat[0]["text"], "文字上下文");
    assert_eq!(chat[2]["image_url"]["url"], images[0].data_url);
    assert_eq!(chat[3]["text"], "【第 3 页页面图像】");

    let responses = responses_input_content("文字上下文", &images);
    assert_eq!(responses[0]["text"], "文字上下文");
    assert_eq!(responses[2]["image_url"], images[0].data_url);
    assert_eq!(responses[3]["text"], "【第 3 页页面图像】");
    assert_eq!(chat_completion_content("纯文字", &[]), json!("纯文字"));
}

#[test]
fn request_fallback_drops_unsupported_inputs_and_preserves_other_errors() {
    assert_eq!(
        request_fallback(reqwest::StatusCode::BAD_REQUEST, true, true, false),
        Some((false, true))
    );
    assert_eq!(
        request_fallback(reqwest::StatusCode::BAD_REQUEST, false, true, true),
        Some((false, false))
    );
    assert_eq!(
        request_fallback(reqwest::StatusCode::BAD_REQUEST, false, true, false),
        None
    );
    assert_eq!(
        request_fallback(reqwest::StatusCode::PAYLOAD_TOO_LARGE, true, true, true),
        Some((true, false))
    );
    assert_eq!(
        request_fallback(
            reqwest::StatusCode::UNPROCESSABLE_ENTITY,
            false,
            true,
            false
        ),
        None
    );
    assert_eq!(
        request_fallback(reqwest::StatusCode::BAD_REQUEST, false, false, false),
        None
    );
}
