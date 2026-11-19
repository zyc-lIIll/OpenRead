# Agent 运行时

`agent/` 放共享的 Agent 运行循环和上下文处理，包括上下文整理、提示词注入、技能、模型请求和流式输出。具体的讲解和批注工作流分别放在 `explanation/` 与 `annotation/`。

初始实现先完成最小循环：接收聊天输入和所选模型，调用统一的模型路由，将流式内容交给前端，并明确区分正常结束、停止和错误。账号授权与 API Key 配置由 `ai-verification/` 和 `providers/` 负责；Agent 不持有 UI 业务状态。
