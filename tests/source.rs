use super::*;

#[test]
fn imports_same_name_pdfs_as_separate_copies_and_preserves_originals() {
    let first_dir = tempfile::tempdir().unwrap();
    let second_dir = tempfile::tempdir().unwrap();
    let first = first_dir.path().join("paper.PDF");
    let second = second_dir.path().join("paper.PDF");
    fs::write(&first, b"%PDF-1.7 first").unwrap();
    fs::write(&second, b"%PDF-1.7 second").unwrap();
    let workspace = Workspace::new().unwrap();
    let a = workspace.import(&first).unwrap();
    let b = workspace.import(&second).unwrap();
    assert_ne!(a.id, b.id);
    assert_eq!(a.name, b.name);
    assert_eq!(
        workspace
            .source_name(Uuid::parse_str(&a.id).unwrap())
            .unwrap(),
        "paper.PDF"
    );
    let copy = workspace.root.path().join(&a.id);
    assert_eq!(fs::read(&copy).unwrap(), b"%PDF-1.7 first");
    workspace.remove(Uuid::parse_str(&a.id).unwrap()).unwrap();
    assert!(workspace
        .source_name(Uuid::parse_str(&a.id).unwrap())
        .is_err());
    assert!(!copy.exists());
    assert!(workspace.root.path().join(&b.id).exists());
    workspace.cleanup();
    assert!(!workspace.root.path().exists());
    assert_eq!(fs::read(&first).unwrap(), b"%PDF-1.7 first");
    assert_eq!(fs::read(&second).unwrap(), b"%PDF-1.7 second");
}

#[test]
fn rejects_unsupported_extensions_and_missing_files() {
    let dir = tempfile::tempdir().unwrap();
    let text = dir.path().join("notes.txt");
    fs::write(&text, b"%PDF-1.7").unwrap();
    let workspace = Workspace::new().unwrap();
    assert!(workspace.import(&text).is_err());
    assert!(workspace.import(&dir.path().join("missing.pdf")).is_err());
    assert_eq!(fs::read_dir(workspace.root.path()).unwrap().count(), 0);
}
