use super::source::pdf::page_range;
use serde_json::json;

#[test]
fn source_read_limits_are_inclusive_and_max_five_pages() {
    assert_eq!(
        page_range(&json!({"page_start":2,"page_end":6})).unwrap(),
        (2, 6)
    );
    assert!(page_range(&json!({"page_start":2,"page_end":7})).is_err());
    assert!(page_range(&json!({"page_start":0,"page_end":1})).is_err());
}
