# Agent 工具注册与调度

`tool/` 提供跨 Agent 共用工具和注册类型。各 Agent 在自己的 loop 中组合共用工具与业务工具，并决定每轮开放哪些工具。

每个注册项绑定 function schema 和处理函数。Agent 从注册表选择本轮工具子集，用该子集生成请求 `tools`，并将同一子集交给 `tool::dispatch` 执行校验，避免模型调用当前请求未开放的工具。处理函数负责验证 JSON 参数和业务边界，并返回统一工具结果、引用及动作；工具结果由 Agent 追加为 `function_call_output`。

当前共用注册表提供：

- `list_sources` 列出当前已导入 PDF 及页数。
- `get_overview` 读取 Source 当前运行期的 Markdown 概览；缺少概览时明确返回未初始化。
- `read_source` 通过 PDF.js 桥读取原文并提供引用编号。每次最多 5 页，所有 Source ID 均按 UUID 和 Workspace 校验。

工具输出上限为 48,000 字节。参数不能包含任意文件路径，工具也不直接读取原始路径。未注册或未包含在当前请求子集中的工具不会执行。

Explanation 自己管理 `select_workflow` 的首轮要求，以及 `organize_output` / `continue_output` / `continue_collection` 阶段控制；这些规则不属于共用工具注册器。`continue_output` 留在正式输出阶段继续当前片段，`continue_collection` 则回到取材阶段。`open_explanation` 是用户确认后由 ChatKit 调用的独立 action，不暴露给模型。

`overview.rs` 和 `source/` 分别实现概览与格式专用读取器。当前只支持 PDF Source 读取；Markdown 导入、OCR、区域锚点和持久化尚未实现。
