# Annotation Agent

该目录负责批注浮窗中的问答上下文和工作流。上下文可以是选中文字或公式、PDF/PPT 当前页，或 PDF 框选截图；后续回复可以引用父讨论中的具体回复。

批注类型只有 `comment` 和 `question`。锚点和父子关系由 `desktop/backend/annotation/` 保存；本目录复用 `../` 的最小流式 Agent 循环。
