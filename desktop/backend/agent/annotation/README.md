# Annotation Agent（未实现）

这是目标模块，当前没有 Annotation Agent 问答或自动便签工作流。PDF 选区/截图接口和基础聊天服务尚未连接成 Annotation 功能。

目标上下文包括选中文字或公式、PDF/PPT 当前页、PDF 框选截图，回复可继续引用父讨论中的具体回复。类型只有 `comment` 和 `question`，但用户不必先选类型；Agent 应在对话中自动整理便签。锚点、便签、完整讨论及父子关系的持久化方案由 `desktop/backend/annotation/` 和 `acr/` 配合落实。
