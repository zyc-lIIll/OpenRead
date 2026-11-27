pub(super) fn load_markdown(key: &str) -> Result<&'static str, String> {
    match key {
        "skill/explanation/source_explanation.md" => {
            Ok(include_str!("../skill/explanation/source_explanation.md"))
        }
        "skill/explanation/knowledge_question.md" => {
            Ok(include_str!("../skill/explanation/knowledge_question.md"))
        }
        "skill/explanation/reading_discussion.md" => {
            Ok(include_str!("../skill/explanation/reading_discussion.md"))
        }
        "skill/explanation/review.md" => Ok(include_str!("../skill/explanation/review.md")),
        "skill/explanation/other.md" => Ok(include_str!("../skill/explanation/other.md")),
        "skill/math.md" => Ok(include_str!("../skill/math.md")),
        "skill/formal_paper.md" => Ok(include_str!("../skill/formal_paper.md")),
        "skill/empirical.md" => Ok(include_str!("../skill/empirical.md")),
        "skill/humanities.md" => Ok(include_str!("../skill/humanities.md")),
        _ => Err("Markdown 指引无效。".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::load_markdown;

    #[test]
    fn loads_only_allowlisted_documents() {
        for key in [
            "skill/explanation/source_explanation.md",
            "skill/explanation/knowledge_question.md",
            "skill/explanation/reading_discussion.md",
            "skill/explanation/review.md",
            "skill/explanation/other.md",
            "skill/math.md",
            "skill/formal_paper.md",
            "skill/empirical.md",
            "skill/humanities.md",
        ] {
            assert!(!load_markdown(key).unwrap().trim().is_empty(), "{key}");
        }

        assert!(load_markdown("skill/../../README.md").is_err());
        assert!(load_markdown("skill/missing.md").is_err());
    }
}
