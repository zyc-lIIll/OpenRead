# API 服务商与模型

`providers` 管理用户配置的 API 服务商、API Key、模型验证和模型请求。配置保存在本机 JSON 中，可同时包含多个服务商。每个服务商包含稳定 ID、名称、Base URL、API Key 和模型 ID 列表；用户可在设置中编辑名称和地址、增删服务商及模型。

前端只显示 API Key 的星号遮罩，不返回具体 Key 给界面。遮罩只控制显示，不加密本机 JSON 文件；不得把真实 Key 写入 Git、ACR、日志或样例。

聊天模型选择器显示「服务商名称 + 模型名称」，请求使用服务商 ID、Base URL、Key 和模型 ID 路由。ChatGPT 授权请求由 `ai-verification` 提供，但两种渠道复用同一套聊天入口和交互。登录 ChatGPT 后也可切换为优先使用 API 服务商。
