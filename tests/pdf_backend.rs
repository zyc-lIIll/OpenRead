use super::*;
use std::fs;

fn imported_workspace() -> (tempfile::TempDir, Workspace, String) {
    let dir = tempfile::tempdir().unwrap();
    let pdf = dir.path().join("sample.pdf");
    fs::write(&pdf, b"%PDF-1.7 sample bytes").unwrap();
    let workspace = Workspace::new().unwrap();
    let entry = workspace.import(&pdf).unwrap();
    (dir, workspace, entry.id)
}

#[test]
fn reads_bounded_ranges_only_for_imported_ids() {
    let (_dir, workspace, source_id) = imported_workspace();
    let id = Uuid::parse_str(&source_id).unwrap();
    assert_eq!(workspace.file_size(id).unwrap(), 21);
    assert_eq!(workspace.read_range(id, 5, 6).unwrap(), b"1.7 sa");
    assert!(workspace.read_range(id, 0, 0).is_err());
    assert!(workspace.read_range(id, 0, 1024 * 1024 + 1).is_err());
    assert!(workspace.read_range(id, 19, 3).is_err());
    assert!(workspace.read_range(Uuid::new_v4(), 0, 1).is_err());
    assert!(workspace.read_range(id, u64::MAX, 1).is_err());
}

#[test]
fn parser_responses_must_match_the_pending_request_and_result_kind() {
    let bridge = ParserBridge::default();
    let request_id = Uuid::new_v4();
    let source_id = Uuid::new_v4();
    let (reply, _receiver) = oneshot::channel();
    bridge.state.lock().unwrap().pending.insert(
        request_id,
        Pending {
            source_id,
            page_number: Some(2),
            kind: ParseKind::PageText,
            reply,
        },
    );

    let mismatch = CompletionMessage {
        request_id: request_id.to_string(),
        source_id: Uuid::new_v4().to_string(),
        page_number: Some(2),
        kind: ParseKind::PageText,
        result: WireResult {
            ok: true,
            data: Some(ParseData::Page {
                text: "page".into(),
                view: [0.0, 0.0, 100.0, 100.0],
                rotation: 0,
                user_unit: 1.0,
            }),
            error: None,
        },
    };
    assert!(bridge.complete(mismatch).is_err());
    assert!(bridge
        .state
        .lock()
        .unwrap()
        .pending
        .contains_key(&request_id));

    let wrong_kind = CompletionMessage {
        request_id: request_id.to_string(),
        source_id: source_id.to_string(),
        page_number: Some(2),
        kind: ParseKind::PageText,
        result: WireResult {
            ok: true,
            data: Some(ParseData::Document { page_count: 2 }),
            error: None,
        },
    };
    assert!(bridge.complete(wrong_kind).is_err());
    assert!(bridge
        .state
        .lock()
        .unwrap()
        .pending
        .contains_key(&request_id));
}

#[test]
fn validates_one_based_pages_and_finite_page_geometry() {
    let valid = ParseData::Page {
        text: "hello".into(),
        view: [0.0, 0.0, 612.0, 792.0],
        rotation: 90,
        user_unit: 1.0,
    };
    assert!(validate_parse_data(Some(1), ParseKind::PageText, &valid).is_ok());
    assert!(validate_parse_data(Some(0), ParseKind::PageText, &valid).is_err());
    assert!(validate_parse_data(None, ParseKind::PageText, &valid).is_err());
    assert!(validate_parse_data(
        None,
        ParseKind::Document,
        &ParseData::Document { page_count: 0 }
    )
    .is_err());
    assert!(validate_parse_data(
        Some(1),
        ParseKind::PageText,
        &ParseData::Page {
            text: String::new(),
            view: [0.0, 0.0, f64::INFINITY, 10.0],
            rotation: 0,
            user_unit: 1.0,
        }
    )
    .is_err());
    assert!(validate_parse_data(
        Some(1),
        ParseKind::PageText,
        &ParseData::Page {
            text: String::new(),
            view: [0.0, 0.0, 10.0, 10.0],
            rotation: 45,
            user_unit: 1.0,
        }
    )
    .is_err());
}

#[test]
fn page_image_results_must_be_bounded_png_data_urls() {
    let valid = ParseData::PageImage {
        data_url: format!("{PAGE_IMAGE_DATA_URL_PREFIX}iVBORw0KGgo="),
    };
    assert!(validate_parse_data(Some(1), ParseKind::PageImage, &valid).is_ok());
    assert!(validate_parse_data(Some(0), ParseKind::PageImage, &valid).is_err());
    assert!(validate_parse_data(Some(1), ParseKind::PageText, &valid).is_err());
    assert!(validate_page_image("data:image/jpeg;base64,iVBORw0KGgo=").is_err());
    assert!(validate_page_image("data:image/png;base64,Zm9v").is_err());
    let oversized = format!(
        "{PAGE_IMAGE_DATA_URL_PREFIX}{}",
        "a".repeat(PAGE_IMAGE_BYTE_LIMIT.div_ceil(3) * 4 + 1)
    );
    assert!(validate_page_image(&oversized).is_err());
}

#[test]
fn deserializes_frontend_camel_case_parse_payloads() {
    let document: CompletionMessage = serde_json::from_str(
        r#"{"requestId":"8e60f845-a50c-46e6-8aa2-b61788e8f2c2","sourceId":"c412e2ee-c62c-4788-8434-cfa3d5f0c5db","pageNumber":null,"kind":"document","result":{"ok":true,"data":{"pageCount":4}}}"#,
    )
    .unwrap();
    assert!(matches!(
        document.result.data,
        Some(ParseData::Document { page_count: 4 })
    ));

    let page: CompletionMessage = serde_json::from_str(
        r#"{"requestId":"8e60f845-a50c-46e6-8aa2-b61788e8f2c2","sourceId":"c412e2ee-c62c-4788-8434-cfa3d5f0c5db","pageNumber":3,"kind":"pageText","result":{"ok":true,"data":{"text":"example","view":[0,0,612,792],"rotation":0,"userUnit":1}}}"#,
    )
    .unwrap();
    assert!(matches!(page.result.data, Some(ParseData::Page { .. })));

    let image: CompletionMessage = serde_json::from_str(&format!(
        r#"{{"requestId":"8e60f845-a50c-46e6-8aa2-b61788e8f2c2","sourceId":"c412e2ee-c62c-4788-8434-cfa3d5f0c5db","pageNumber":3,"kind":"pageImage","result":{{"ok":true,"data":{{"dataUrl":"{}"}}}}}}"#,
        format!("{PAGE_IMAGE_DATA_URL_PREFIX}iVBORw0KGgo=")
    ))
    .unwrap();
    assert!(matches!(
        image.result.data,
        Some(ParseData::PageImage { .. })
    ));
}

#[test]
fn removing_source_cancels_its_pending_parse_requests() {
    let (_dir, workspace, source_id) = imported_workspace();
    let source_uuid = Uuid::parse_str(&source_id).unwrap();
    let bridge = ParserBridge::default();
    let request_id = Uuid::new_v4();
    let (reply, mut receiver) = oneshot::channel();
    bridge.state.lock().unwrap().pending.insert(
        request_id,
        Pending {
            source_id: source_uuid,
            page_number: None,
            kind: ParseKind::Document,
            reply,
        },
    );

    bridge.remove_source(&workspace, source_uuid).unwrap();
    assert!(!workspace.root.path().join(&source_id).exists());
    assert!(bridge.state.lock().unwrap().pending.is_empty());
    assert!(matches!(
        receiver.try_recv(),
        Ok(Err(reason)) if reason == "Source 已移除。"
    ));
}

#[test]
fn parse_timeout_removes_the_pending_request() {
    let (_dir, workspace, source_id) = imported_workspace();
    let bridge = ParserBridge::default();
    bridge.connect(Channel::new(|_| Ok(()))).unwrap();
    let source_id = Uuid::parse_str(&source_id).unwrap();

    let result = tauri::async_runtime::block_on(bridge.request_with_timeout(
        &workspace,
        source_id,
        None,
        ParseKind::Document,
        std::time::Duration::from_millis(1),
    ));
    assert!(matches!(result, Err(reason) if reason == "PDF 解析超时。"));
    assert!(bridge.state.lock().unwrap().pending.is_empty());
}
