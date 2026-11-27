# Source 初始化 Agent

`initial/` 将已导入的 PDF 逐段提供给模型，持续更新一份 Markdown 阅读概览。概览提炼文章结构、章节作用、概念关系、页码和精读关注点，帮助后续阅读和 Explanation 定位原文。它不等同于 Explanation，也不替代原文。

## 调用接口与数据流

前端从 `app_ui/reader.tsx` 调用 `initialize_source`，传 Source ID、模型路由 ID、默认 ChatGPT 思考强度和进度通道。后端通过 `Workspace` 校验 Source，不接受任意文件路径。`cancel_source_initialization` 按 Source 取消正在进行的任务。

PDF 页码与页面图像由 `agent/tool/source/pdf.rs` 和 `source/PDF/` 桥接取得。文字和 PNG 图像通过 `agent/model/` 统一请求送给模型。初始化自己处理分段、图像回退、概览压缩、进度、取消和结构化结果，不进入对话 Agent 的双阶段 loop 或 `CompletionEvent`。

成功返回 Markdown 概览、搜索来源和阅读限制说明。概览保存在前端阅读器与后端 `Workspace` 内存中，可由 Explanation 的 `get_overview` 工具按需读取。移除 Source 或退出程序时清除，当前不写文件或 ACR。

## 分段与模型请求

每段最多 12,000 个 Unicode 字符和 8 张页面图像。单张 PNG 最长边不超过 2,048 px、大小不超过 4 MiB。页边界模型可建议下一段连续页码，后端会校验起点和范围，缺失或无效时回到本机默认切分。模型每轮结合上一版概览与新材料返回完整概览，正文最多 6,000 字符；超限后额外尝试压缩一次，仍超限则报错。

自动思考强度省略 Responses API 字段；低、中、高只用于 ChatGPT 路由。ChatGPT 可按需调用内建搜索，搜索来源单独返回。通用 API 路由没有搜索功能，会说明未检索外部资料。

## 图像、失败和取消

扫描页没有文字层时仍将页面图像交给支持视觉输入的模型，本模块不做本地 OCR。页面图像无法生成时尽量继续使用其他页和可提取文本，并记录缺失页。若服务商拒绝图像输入，工作流会在适用时改用文字请求并说明未纳入的页码。无可读文字和可用图像的末段会标注未覆盖页码；取消返回 `cancelled`，不保存未完成概览。

## 子模块与状态

`initial/` 当前只有 `mod.rs`，其中包含初始化 Tauri 命令、任务协调、提示词、分段请求、压缩、回退、取消和进度处理。Source 通用工具及 PDF 上下文分段器分别位于父级 `agent/tool/` 与 `agent/tool/source/`。该流水线已实现，Markdown 概览由前端安全渲染，跨会话保存与 ACR 集成尚未实现。
