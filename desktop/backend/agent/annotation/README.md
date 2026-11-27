# Annotation Agent 规划

`annotation/` 预留给原文、Explanation 文档和讨论回复上的局部对话 Agent。当前目录只有本说明，没有 loop、工具或运行时代码。

## 目标上下文与便签

Annotation 可从完整文字/公式选区、PDF 或未来 PPT 当前页、PDF 矩形框选开始。它需要接收可定位的原文锚点；框选还携带截图。Agent 支持 `comment` 与 `question` 两类便签，但用户不必预先选择类型。讨论过程中由 Agent 整理便签，便签关联回完整讨论。回复可引用父讨论中的具体消息并继续追问。

## 共用接口和独立职责

Annotation 应复用 `agent/model/` 的统一请求与归一化响应、`CompletionEvent` 的双阶段显示公约、`agent/citation.rs` 的引用解析、`agent/tool/` 的共用注册机制，以及 `frontend/agent_ui/chat_ui/` 的聊天呈现。它构造统一模型请求并消费模型层返回的 `Turn`，不直接处理服务商协议；它自行定义工具白名单、上下文、loop、阶段与回复结束规则。显示事件沿用共用接口，不能把工具参数、工具结果或模型 `Context` 当作 Activity。Explanation 的 `select_workflow` 与阶段控制函数不属于共用接口。

未来实现时，loop 放在 `agent/annotation/agent_loop.rs`；Annotation 专属工具放在 `agent/tool/annotation/`，专属长指引放在 `agent/skill/annotation/`。`agent/tool/` 与 `agent/skill/` 根目录只放可共用项。长指引写在 Markdown 中，由后端白名单加载；短固定 prompt 留在实际调用它的程序内。

Source 锚点与浮窗屏幕位置分别保存。Annotation 关系和持久化归未来的 `desktop/backend/annotation/`；ACR 档案关系见[顶层 ACR 草案](../../../../ACR/README.md)。后端数据结构、前端浮窗和 Agent 工作流尚未实现。
