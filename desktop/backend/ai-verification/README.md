# ChatGPT 授权

`ai-verification` 负责 ChatGPT 账号登录、授权状态和当前账号信息。当前只支持一个已登录的 ChatGPT 账号，并向设置页提供账号标识。

登录 ChatGPT 后默认使用 ChatGPT 额度；用户仍可在设置里切换为优先使用 API 服务商。账号渠道与服务商渠道接入同一套聊天交互，渠道只影响请求路由。

ChatGPT 可用模型和思考强度由聊天模型选择器呈现。API 服务商的配置、Key 和模型调用归 `providers` 管理。
