# 模型协议适配

`model/` 是 Agent 工作流连接模型服务的共享适配层。调用方使用共同的消息、文本增量、完整响应和工具调用结构；`chat_completions.rs` 与 `responses.rs` 独立处理服务端 wire 格式、工具/消息序列化以及 SSE 解码。`initial/` 和 `explanation/` 不各自解析供应商的流格式。

Responses 请求保持 `store: false`。工具轮次会把 Responses 返回的原始 output items 原序续传，以保留无状态推理上下文；Chat Completions 则用标准 assistant tool call 和 tool result 消息。ChatGPT 额度路由的本机函数工具按 Responses namespace 注册，API 服务商路由使用 Chat Completions function 工具结构。两种协议的 SSE 字节流统一按边界解析并限制单帧大小；模型输出达到 token 上限时统一报告可续写状态。

当前模型路由由 `agent/mod.rs` 根据本机服务商设置或 ChatGPT 授权建立。API Key 和访问令牌不进入统一消息对象、日志或前端。
