# Source 初始化

`initial/` 负责把已导入的 Source 分段交给模型，持续更新一份供后续精读使用的 Markdown 概览。当前只支持 PDF。它不负责文件导入、PDF 解析、Markdown 渲染或持久化；对应职责分别在 `desktop/backend/source/`、`desktop/backend/agent/source/` 和前端阅读器中。

## 入口与流程

`initialize_source` 接收 Source ID、模型路由 ID、ChatGPT 默认思考强度和进度通道，不接收任意文件路径。后端先通过 `Workspace` 校验 Source，再读取 PDF 页数、逐页文字和页面 PNG。页面上下文及分段规则由 `desktop/backend/agent/source/pdf.rs` 管理，PDF.js 解析桥接位于 `desktop/backend/source/PDF/`。自动模式省略 `reasoning.effort`；低、中、高仅用于 ChatGPT 路由，API Key 请求不带该参数。

每轮模型请求都携带固定 system instructions、上一轮完整概览和当前片段。模型需要返回完整 Markdown 概览及下一部分关注点；只有页边界处才允许建议后续连续页码。后端会校验建议从首个未读页开始、范围不超过 8 页，并从 Markdown 中移除隐藏页码标记。建议缺失或无效时按字符数和页数默认切分。

概览正文上限为 6,000 个 Unicode 字符。超出时会额外请求模型压缩一次；压缩后仍超限则返回错误。中间片段可以按模型判断使用联网搜索；ChatGPT 路由提供 `web_search`，API Key 路由不提供联网工具。最后一个可读片段会得到整理完整概览并酌情补充搜索的提示。搜索来源与概览 Markdown 分开返回。

## PDF 图片与错误处理

PDF 页面文字和 PNG 图像在同一轮请求中发送。每段最多 12,000 个 Unicode 字符和 8 张页面图像。页面 PNG 最长边不超过 2,048 px，每张不超过 4 MiB。无文字层的页面仍可由视觉模型根据页面图像阅读；本模块不运行本地 OCR。

单页图像无法生成时，尽可能使用其余页面和可提取文字，并在 `notes` 中列出缺图页。服务商明确拒绝图像输入或携图请求体过大时，会移除图像重试，同时告知模型被省略的页码；模型配置等无关错误仍作为失败返回。若最后一段没有可读文字或可用图像，会基于此前概览完成收尾，并注明未覆盖页码。

## 返回与生命周期

成功时返回 Markdown、联网引用和阅读限制说明；取消时返回 `cancelled`。初始化期间通过进度通道报告当前页和阶段，`cancel_source_initialization` 可取消任务。当前结果仅由前端阅读器保存在内存中，不写入 ACR 或本地文件；退出应用或移除 Source 后清除。

提示词、模型请求、页码建议验证和概览压缩均在 `mod.rs`。格式相关的上下文获取与分段规则留在 `agent/source/`，避免把 PDF 专属逻辑混入初始化工作流。
