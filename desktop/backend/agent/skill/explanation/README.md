# Explanation 工作流提示词

`agent/skill/explanation/` 保存 `select_workflow` 可选择的五份任务指引：Source 讲解、知识问题、伴读讨论、复盘和其他。模型根据用户消息选择工作流；handler 按首轮返回的枚举加载对应的一份，避免每次模型请求都携带全部工作流内容。

Explanation 专属 handler 位于 [`agent/tool/explanation/`](../../tool/explanation/README.md)。它依据 `workflow` 枚举装载一份任务指引，并按 `reading_lens` 添加共用阅读视角：`general` 使用 handler 内的短固定文字，其他视角从 `agent/skill/` 根目录加载 Markdown。它们是基础 instructions 的动态补充，不负责 Agent loop。

全局角色、安全边界、引用约定和 Source 使用规则见 [`../../explanation/system.md`](../../explanation/system.md)。共用阅读视角见 [`../README.md`](../README.md)。Markdown 指引由编译期白名单加载，当前没有用户编辑和覆盖 UI。
