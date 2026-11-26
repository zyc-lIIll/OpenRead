# Explanation 主对话 Agent

`explanation/` 是阅读台右侧主对话的 Agent，由 ChatKit 经 [`../chatkit.rs`](../chatkit.rs) 接入。循环主体位于 `agent_loop.rs`，由 Explanation 自己负责首轮、阶段转换、工具调用、上下文和结束处理；不调用通用 Agent runner。

## 首轮与提示词

每条新用户消息首轮要求注册表中的 `select_workflow`。它选择五类工作流之一、一个学科阅读视角，并返回 `suggest_new_explanation: bool`；不生成会话标题。会话层根据首条用户输入生成标题。该首轮规则只属于 Explanation，不是其他 Agent 的通用要求。

`skill.rs` 将选择映射到已有 workflow prompt 和学科 skill；`system.md` 提供 Explanation 基本规则。`select_workflow` 的处理器把所选 prompt 作为指令动作返回，由 Explanation loop 应用。它不选择模型协议或解析服务商响应。

## 取材与正式输出

- **阶段 1 · 取材**：首轮选择 workflow 后，按需调用 Source 工具和适配器支持的内建搜索。过程文字、搜索进度与实际工具状态通过共用回复事件临时显示。本机函数由 Explanation loop 选择本轮工具清单，经注册表校验后交给 `tool::dispatch`；结果作为 `function_call_output` 加入 input。
- **阶段 2 · 正式输出**：Explanation loop 先清除当前片段的临时播报，再从首个正文增量起写入同一条助手消息。模型可调用 `continue_output` 留在阶段 2 继续当前片段，或调用 `continue_collection` 完成当前片段后回到取材阶段；同一响应若请求两种控制，会收到冲突错误并留在阶段 2。正常结束且没有控制调用时完成整条回复。因服务商输出长度上限而截断时，loop 会自动留在当前阶段续写。
- **切换校验**：只有完整响应中的控制函数调用且参数校验通过，才会改变下一次请求。普通文字 chunk、JSON 中间片段和未完成参数不会触发切换。取材正常结束未切换时，下一轮只要求 `organize_output`，再失败则报错。
- **上下文与限制**：同一次运行跨阶段保留完整 input、助手过程文字、函数调用及结果，并原样传递模型层不透明 Context。沿用最多 24 轮请求、40 次工具调用和 8 次连续输出截断续写的上限；主动调用 `continue_output` 受 24 轮请求总上限约束。

两阶段流向是 Explanation 专属行为。对话 Agent 共用的临时播报、正式正文和片段完成事件见 [Agent 总览](../README.md)；流式引用解析由共用 [`../citation.rs`](../citation.rs) 负责。供应商协议和任意文本 chunk 的含义见 [模型 README](../model/README.md)。

## 新建 Explanation 建议

`select_workflow` 返回真时，当前回复结束后显示新开对话选项。`open_explanation` 是独立 ChatKit 用户 action，不会作为模型函数传入请求。后端将一次性建议绑定到当前线程最新用户消息，10 分钟后失效；确认后只返回该条用户原文供前端预填新线程草稿，用户点击发送后才创建服务端线程。拒绝、过期、重复调用或线程已有新消息都不会切换对话。

## 当前限制

- Source 工具与引用锚点目前只支持已导入 PDF 和 PDF 页码；Markdown 文件导入尚未实现。
- 线程、建议状态和对话内容只存在当前进程内存中；应用重启后不能恢复。当前不实现 Annotation、跨会话持久化、Explanation 文档版本或 ACR。
- 当前 Search 仅由 ChatGPT 路由提供；通用 API 路由明确没有搜索工具实现。
