use super::*;

#[test]
fn page_parts_keep_page_anchors_and_respect_the_segment_limit() {
    let parts = page_parts(7, &"字".repeat(SEGMENT_CHAR_LIMIT + 100));
    assert!(parts.len() > 1);
    assert!(parts
        .iter()
        .all(|part| part.text.chars().count() <= SEGMENT_CHAR_LIMIT));
    assert!(parts[0].text.starts_with("【第 7 页】"));
    assert!(parts[1].text.starts_with("【第 7 页（续）】"));
    assert!(parts.iter().all(|part| part.has_text));
    assert!(!parts[0].last_part);
    assert!(parts.last().unwrap().last_part);
    assert_eq!(
        parts
            .iter()
            .map(|part| part.text.matches('字').count())
            .sum::<usize>(),
        SEGMENT_CHAR_LIMIT + 100
    );
}

#[test]
fn pages_without_text_still_make_a_context_part_for_their_image() {
    let parts = page_parts(4, " \n ");
    assert_eq!(parts.len(), 1);
    assert!(parts[0].text.contains("本页无可提取文字"));
    assert!(!parts[0].has_text);
    assert!(parts[0].last_part);
}

#[test]
fn segment_builder_keeps_one_image_per_page_and_respects_page_and_text_limits() {
    let mut builder = SegmentBuilder::default();
    let image = "data:image/png;base64,iVBORw0KGgo=";
    let first_parts = page_parts(1, "first page");
    builder.push(first_parts[0].clone(), Some(image), false);
    builder.push(
        PagePart {
            page_number: 1,
            text: "same page continuation".into(),
            last_part: true,
            has_text: true,
        },
        Some(image),
        false,
    );
    for page in 2..=8 {
        let part = page_parts(page, "page").remove(0);
        assert!(!builder.would_exceed(&part));
        builder.push(part, Some(image), false);
    }
    assert_eq!(builder.images.len(), 8);
    let ninth = page_parts(9, "ninth").remove(0);
    assert!(builder.would_exceed(&ninth));

    let segment = builder.finish().unwrap();
    assert_eq!(segment.page_range, PageRange { start: 1, end: 8 });
    assert_eq!(segment.images.len(), 8);
    assert!(segment.has_text);
    assert!(segment.last_page_complete);
    assert!(builder.is_empty());
}

#[test]
fn segment_builder_splits_before_the_character_limit() {
    let mut builder = SegmentBuilder::default();
    let first = page_parts(1, &"a".repeat(SEGMENT_CHAR_LIMIT / 2)).remove(0);
    let second = page_parts(2, &"b".repeat(SEGMENT_CHAR_LIMIT / 2)).remove(0);
    builder.push(first, None, true);
    assert!(builder.would_exceed(&second));
    let segment = builder.finish().unwrap();
    assert_eq!(segment.image_failures, vec![1]);
    assert!(segment.images.is_empty());
    assert_eq!(
        segment.text.chars().count(),
        SEGMENT_CHAR_LIMIT / 2 + "【第 1 页】\n".chars().count()
    );
}

#[test]
fn page_range_marker_is_removed_and_malformed_markers_are_ignored() {
    let (markdown, range, found) = extract_page_range(
        "# 概览\n\n## 下一部分关注点\n看研究方法。建议第 5-7 页。\n<!-- openread-next-pages: 5-7 -->",
    );
    assert!(found);
    assert_eq!(range, Some(PageRange { start: 5, end: 7 }));
    assert!(!markdown.contains("openread-next-pages"));
    assert!(markdown.contains("建议第 5-7 页"));

    let (markdown, range, found) = extract_page_range("# 概览\n<!-- openread-next-pages: 4-x -->");
    assert!(found);
    assert_eq!(range, None);
    assert!(!markdown.contains("openread-next-pages"));
}

#[test]
fn recommendation_must_start_at_the_next_unread_page_and_fit_the_request_cap() {
    assert!(valid_recommendation(PageRange { start: 6, end: 13 }, 6, 13));
    assert!(!valid_recommendation(PageRange { start: 7, end: 8 }, 6, 10));
    assert!(!valid_recommendation(
        PageRange { start: 6, end: 14 },
        6,
        14
    ));
    assert!(!valid_recommendation(
        PageRange { start: 6, end: 13 },
        6,
        12
    ));
}

#[test]
fn page_ranges_are_sorted_deduplicated_and_compact() {
    assert_eq!(format_page_ranges(&[9, 3, 2, 3, 1, 6, 8]), "1-3、6、8-9");
    assert_eq!(format_page_ranges(&[]), "");
}
