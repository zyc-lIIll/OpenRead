# Explanation 主对话 Agent（首版已实现）

`explanation/` 是阅读台右侧主对话的 Agent 工作流，ChatKit 请求从 [`../chatkit.rs`](../chatkit.rs) 进入。它复用 [`../model/`](../model/README.md) 的统一模型流接口和 [`../tool/`](../tool/README.md) 的 Source 工具，不直接拼接供应商请求体。

## 一轮对话

1. 加入全局 `system.md` 和当前 ChatKit 线程的可见文本历史。
2. 通过 API 请求的 `tools` schema 要求模型调用 `select_workflow`，选择五种工作流之一、一个学科视角并给对话命名。
3. `skill.rs` 只加载所选任务 prompt 和学科 Markdown skill。
4. 模型可循环调用 Source 工具。每个工具调用经过名称、JSON 参数、Source UUID、页码范围和结果大小验证。一个回答最多进行 24 轮模型请求、40 次工具调用；遇到模型输出长度上限时，同一回答最多自动续写 8 次，超出后明确报错。
5. 最终文本增量继续经 ChatKit SSE 显示；`[[cite:rN]]` 在流式中隐藏，并于完成时校验为当前回合实际读取过的 PDF 页，再生成可点击 EntitySource 注释。

## Prompt 与工具的区别

`system.md` 固定 Agent 职责、安全与证据规则、工作流选择过程及引用协议；`skill/` 按类别补充任务操作指引；`../skill/` 提供选定的学科阅读视角。跨主题或从讨论转完整 Source 讲解时，模型可以建议另开 Explanation，但不能替用户创建。让模型真正知道可调用的工具，需要在每个 API 请求的 `tools` 字段附上函数名、说明和 JSON Schema。system prompt 内的工具注册表便于模型理解职责，但它本身不能启用工具。

Source 工具可以列出当前 PDF、读取可选概览和按页读原文。概览不存在时仍允许读取正文。ChatGPT Responses 路由提供可选 `web_search`；API 服务商路由无通用联网工具。

## 当前限制

- 左右栏仍是一条基础 ChatKit 线程，没有 Explanation 文档版本、分章节导航、跨线程 Annotation 回顾或 ACR 持久化。复盘只读取当前线程可见历史，当前没有 Annotation 数据可供汇总。
- 当前 Source 读取、引用锚点只支持 PDF 页码；Markdown 文件导入尚未实现。
- 聊天记录仍在当前进程内存，新的应用运行不能恢复先前对话；后端没有活动标签/当前页信息，用户用“当前这份/这一页”指代时需先澄清。
- 前端 EntitySource 点击会激活 Source 并跳页；标记遵循单次回复 `rN` 编号，不用于 ACR 持久引用。
