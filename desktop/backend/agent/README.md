# 本机聊天服务与 Agent 规划

`agent/` 当前提供 ChatKit 使用的本机文本聊天服务和基础模型请求。它只把聊天消息转成模型请求并流式返回；Agent 的提示词/上下文组装、工具调用、文档阅读、Explanation 生成、Annotation 问答和自动便签均未实现。普通聊天请求不代表产品 Agent 工作流已经完成。

当前聊天不关联文档，也不写入 ACR。线程及消息仅保存在进程内存，应用退出时清空。

启动时 Rust 后端绑定随机端口的 `127.0.0.1` 地址，并生成仅本次启动有效的 Bearer token。`get_chatkit_connection` 返回本机 `/chatkit` 地址和 token；`list_chat_models` 按 `api` 或 `chatgpt` 返回显示用名称及不透明路由 ID。前端在 ChatKit `inference_options.model` 中传回路由 ID。后端只接受当前目录中的模型 ID，并从本机服务商配置或 ChatGPT 授权目录解析实际请求地址与凭据；前端不能传入任意 URL 或凭据。

协议目前处理 ChatKit 的 `threads.create`、`threads.add_user_message` 以及读取/删除当前进程内线程所需的基本操作。只支持文本消息、流式文本、正常完成、错误和取消；不支持附件、搜索、Agent 工具或持久化。请求目前没有思考强度参数。API 服务商通过配置的 Base URL 请求 `/chat/completions`；ChatGPT 通过 `/v1/responses` 使用 `store:false` 和流式响应，并等待 `response.completed` 才结束。API Key 和 OAuth token 只在 Rust 后端使用，不写日志。
