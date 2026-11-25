# Agent 工具

`tool/` 放 Explanation 等 Agent 工作流共用的后端工具。它读取已导入 Source、生成的概览和原文上下文；不负责文件导入、底层解析或模型 API 请求。

- `mod.rs` 提供工具 schema、注册/调度和边界校验。当前注册 `list_sources`、`get_overview`、`read_source`。工具定义在模型请求的 `tools` 字段传给模型，system prompt 另行说明工具的使用方式。
- `overview.rs` 通过 Source ID 读取 `Workspace` 中当前运行期的 Markdown 概览。概览缺失时返回 `initialized: false`，不伪造内容。
- `source/` 放各格式上下文读取器。当前只实现 `source/pdf.rs`：通过 `source/PDF` 的 PDF.js 桥逐页读取正文，给每页附带稳定的 PDF 页锚点。一次 `read_source` 最多返回 5 页。

首版工具只针对当前导入 PDF，读取时根据 UUID 校验 Source；不接收文件路径。工具输出上限为 48,000 字节。当前不支持 Markdown 文件导入、OCR、原文页内区域定位或持久化。
