use super::prompt::system_prompt;

#[test]
fn system_prompt_explains_workflow_selection_and_validated_citations() {
    let prompt = system_prompt();
    assert!(prompt.contains("select_workflow"));
    assert!(prompt.contains("[[cite:rN]]"));
    assert!(prompt.contains("不可信数据"));
}
