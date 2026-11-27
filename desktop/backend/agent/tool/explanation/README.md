# Explanation 专属工具

`tool/explanation/` 保存只供 Explanation 使用的模型函数 schema 和 handler。Explanation loop 在首轮注册 `select_workflow`，按阶段开放 `organize_output`、`continue_output` 与 `continue_collection`。

`select_workflow` 校验模型首轮返回的工作流及阅读视角枚举，从 `agent/skill/explanation/` 载入一份任务指引。阅读视角 `general` 使用 handler 内的短固定文字，其他视角从 `agent/skill/` 根目录载入共用 Markdown。`open_explanation` 通过 `ToolAction` 交给 ChatKit，在回复结束后作为用户确认操作显示；它不是模型函数。阶段控制 schema 由此处生成，具体阶段校验与状态转换仍由 `explanation/agent_loop.rs` 执行。

Markdown 仅通过 `agent/tool/markdown.rs` 中的编译期白名单载入。工具 handler 不接受模型或调用方提供的文件路径。工具注册与 handler 约定见[共用工具 README](../README.md)，工作流规则见[Explanation README](../../explanation/README.md)。
