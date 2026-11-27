# Agent Source 上下文读取

`agent/tool/source/` 从已导入的 Source 读取模型需要的原文上下文，并在结果中提供可验证的稳定锚点。底层导入和解析归 `desktop/backend/source/`；这里校验页码范围、管理模型输入片段并生成当前回复可用的引用编号。

## PDF 子模块

当前只有 `pdf.rs`。它通过 PDF parser bridge 获取页数和页面文字，支持 `read_source` 按页读取；同时提供 Source 初始化复用的页码分段、建议校验、缺图范围和字符切分逻辑。页面图像由 `initial/` 在初始化过程中直接调用 `source::pdf::read_page_image` 获取，不经过本模块的读取函数。

PDF 页码从 1 开始且包含封面。`read_source` 单次最多读取 5 页。Source 初始化每段最多 12,000 个 Unicode 字符和 8 张页面图像。页图最大尺寸见 [初始化 README](../../initial/README.md)。输出中的临时引用编号仅在当前对话回复中有效，后续不可持久化为 ACR ID。

未来增加格式模块时只在 `source/` 中组织该格式的 Agent 上下文。Source 列表与概览等通用工具保留在父级 [`tool/`](../README.md)。当前没有其他格式 reader。
