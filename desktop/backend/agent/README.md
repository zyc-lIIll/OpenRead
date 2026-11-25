# 本机聊天服务与 Agent 规划

`agent/` 提供 ChatKit 使用的本机文本聊天服务和基础模型请求，并实现了首版 Source 初始化。普通聊天请求仍只转发聊天消息，不包含 Source 上下文或工具；Explanation 生成、Annotation 问答和自动便签均未实现。基础文本聊天不代表完整 Agent 工作流已经完成。

## Source 初始化（首版已实现）

`agent/initial/` 接收 Source ID、模型路由 ID 和思考强度；不接收任意文件路径。后端通过 `Workspace` 校验导入项；PDF 专属的上下文获取和切分位于 `agent/source/pdf.rs`。它通过 PDF.js 桥接按页读取文字，并请求对应页的 PNG 图像。初始化最多同时运行一项，前端可查看进度并取消。ChatGPT 思考强度使用设置中的默认值，API Key 路由不读取该参数。

每轮最多包含 12,000 个 Unicode 字符和 8 页图像；文字与 PNG 页图在同一个模型请求中发送，页图最长边不超过 2,048 px。无文字层的页面仍以图像提供，不做本地 OCR。页面渲染失败或模型拒绝图像输入时会回退到可用文字，并将受影响页码列入初始化说明。

每轮请求都发送固定 system instructions，以及包含上一版完整概览、本轮新增原文和页码范围的 user input。下一部分关注点保存在上一版 Markdown 中，没有独立字段。页边界处模型可通过 Markdown 隐藏标记建议从第一未读页开始的连续范围，最多 8 页；后端验证后再应用，缺少或无效时按字符数和页数默认切分。模型返回完整更新后的 Markdown 正文，正文上限为 6,000 字符；若超出，额外请求一次压缩，仍超限则报错。阅读限制提示和搜索来源以结构化附加内容返回，不计入正文上限。概览优先保留全文结构、各部分作用、关键概念及关系和原文页码。它是有损导航索引，不替代原文或 Explanation；具体解释和引用仍需回读原文。

初始化提示词位于 `agent/initial/mod.rs`。它把原文和既有概览作为不可信参考资料，要求模型维护层级结构、概念关系、页码和精读关注点；分段处理中逐段指出下一部分关注点。最后一个原文分段带专属 system 指令，要求模型完成概览并判断是否值得补充搜索，然后直接返回最终 Markdown，不再额外调用一次全文整理。固定提示词不包含最终搜索指令。

默认模型和 ChatGPT 默认思考强度分别设置。自动模式不发送 `reasoning.effort`；选择低、中或高时，只给 ChatGPT Responses API 请求附加对应值。普通聊天在聊天框中可覆盖默认值；Source 初始化使用默认值。显式强度请求遇到 HTTP 400 时不自动切换强度，并提示用户检查模型支持情况。API Key 请求不显示该选项、不传参数；初始化 IPC 在 API 路由时使用自动值。

API 路由通过 Chat Completions 更新概览，不提供联网工具，概览会注明未检索外部资料。ChatGPT 路由通过 Responses API 在每个分段请求中提供 `web_search` 和 `tool_choice: auto`，允许模型在中间或最后一个分段自行搜索；提示词在最后一个分段鼓励一次补充搜索，若中间已覆盖则可不重复。搜索来源会单独列出并去重。HTTP 400 时先不带搜索工具重试；服务商明确指出图像输入不受支持，或携图请求返回 HTTP 413 时，会回退到文字请求，并告知模型哪些页图已省略。若最后一段没有可读文字或页面图像，模型会整理现有概览并注明未纳入的页码。其他请求错误仍报告失败。SIWC 是否能搜索仍受模型、账号及组织策略限制。参考 [OpenAI Web Search 文档](https://developers.openai.com/api/docs/guides/tools-web-search) 和 [Sign in with ChatGPT 限制](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations)。

Markdown 概览当前只保存在前端阅读器内存中：返回时保留，移除 Source 或退出应用时清除；不写入 ACR，也不自动注入聊天上下文。概览正文由 `desktop/frontend/source/markdown/MarkdownRenderer.tsx` 渲染，联网来源作为独立的安全链接列表呈现。Markdown Source 文件导入尚未实现；`desktop/backend/source/markdown/` 已提供标题层级、章节范围和图片源范围解析，尚未接入导入与 Annotation 流程。

当前聊天不关联文档，也不写入 ACR。线程及消息仅保存在进程内存，应用退出时清空。

启动时 Rust 后端绑定随机端口的 `127.0.0.1` 地址，并生成仅本次启动有效的 Bearer token。`get_chatkit_connection` 返回本机 `/chatkit` 地址和 token；`list_chat_models` 按 `api` 或 `chatgpt` 返回显示用名称及不透明路由 ID。前端在 ChatKit `inference_options.model` 中传回路由 ID。后端只接受当前目录中的模型 ID，并从本机服务商配置或 ChatGPT 授权目录解析实际请求地址与凭据；前端不能传入任意 URL 或凭据。

协议目前处理 ChatKit 的 `threads.create`、`threads.add_user_message` 以及读取/删除当前进程内线程所需的基本操作。只支持文本消息、流式文本、正常完成、错误和取消；不支持附件、Agent 工具或持久化。普通聊天的思考强度经本地请求头传入并校验；只有 ChatGPT 路由会将非自动值发送为 `reasoning.effort`。API 服务商通过配置的 Base URL 请求 `/chat/completions`；ChatGPT 通过 `/v1/responses` 使用 `store:false` 和流式响应，并等待 `response.completed` 才结束。API Key 和 OAuth token 只在 Rust 后端使用，不写日志。
