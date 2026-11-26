pub struct Selection {
    pub workflow: String,
    pub reading_lens: String,
}

pub fn select(selection: Selection) -> Result<String, String> {
    let workflow = match selection.workflow.as_str() {
        "source_explanation" => include_str!("skill/source_explanation.md"),
        "knowledge_question" => include_str!("skill/knowledge_question.md"),
        "reading_discussion" => include_str!("skill/reading_discussion.md"),
        "review" => include_str!("skill/review.md"),
        "other" => include_str!("skill/other.md"),
        _ => return Err("工作流类型无效。".into()),
    };
    let lens = match selection.reading_lens.as_str() {
        "general" => "使用跨学科的清晰讲解方式，优先定义概念、展示关系和提供必要例子。",
        "math" => include_str!("../skill/math.md"),
        "formal_paper" => include_str!("../skill/formal_paper.md"),
        "empirical" => include_str!("../skill/empirical.md"),
        "humanities" => include_str!("../skill/humanities.md"),
        _ => return Err("学科视角无效。".into()),
    };
    Ok(format!("{}\n\n{}", workflow.trim(), lens.trim()))
}
