# Agent 工具注册与调度

`agent/tool/` 根目录提供多个 Agent 共用的函数注册项及 Source 工具。它负责模型 schema 与本机 handler 的绑定，不决定 Agent 阶段或本轮开放哪些工具。Agent 专属工具放在对应子目录，例如 `explanation/`。

## 工具接口

每个 `ToolDefinition` 包含函数 JSON Schema 和处理函数。Agent loop 从注册表选择本轮工具子集，用同一子集生成请求 `tools` 并限制 `tool::dispatch`。这样模型只能请求本轮已注册的函数。dispatch 检查工具名称与 JSON 格式；具体 handler 反序列化并校验参数、Source ID 和页码。

处理结果以 `ToolExecution` 返回模型可见的工具内容、原文引用和可执行动作。Agent 把工具内容作为 `function_call_output` 加入后续 `input`。模型函数只表达请求；用户确认操作通过 ChatKit 单独触发。

## 当前共用工具

- `list_sources` 列出当前 PDF Source 和页数。
- `get_overview` 获取 Source 的运行期 Markdown 概览，也说明概览是否存在。
- `read_source` 通过 PDF.js 桥按页读取原文并返回本轮引用目标。单次最多 5 页。

所有 Source ID 均按 UUID 并由 `Workspace` 校验。工具不接受任意路径，单次输出不超过 48,000 字节。

Explanation 自己控制 `select_workflow` 首轮选择，以及 `organize_output`、`continue_output`、`continue_collection` 阶段转换。它们是该 Agent 的工作流规则，不属于共用注册表。`open_explanation` 是用户确认后执行的 ChatKit action，不作为模型函数暴露。

## 子目录

- `overview.rs` 当前提供 Source 概览结果封装，由 `get_overview` 使用。
- `source/` 提供按格式读取 Agent 上下文。当前只有 PDF reader，包含页面范围、页码建议和初始化分段逻辑。详细接口见 [Source 上下文 README](source/README.md)。
- `explanation/` 保存 Explanation 专属的 `select_workflow` 和阶段控制工具。详细接口见 [Explanation 工具 README](explanation/README.md)。未来 Annotation 专属工具放在 `annotation/`。
- `markdown.rs` 是内部 helper，按固定 key 的编译期白名单读取 `agent/skill/` 下的 Markdown。它使用 `include_str!`，不读取任意文件路径，也不注册成模型可调用工具。

当前只有 Explanation 使用共用工具注册表。Markdown 导入、OCR、区域锚点和持久化尚未实现。
