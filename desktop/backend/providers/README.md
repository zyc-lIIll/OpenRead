# API 服务商与模型

`providers` 管理用户配置的 API 服务商、API Key 和模型 ID。配置保存在本机 JSON 中，可同时包含多个服务商。每个服务商包含稳定 ID、名称、Base URL、API Key 和模型 ID 列表；用户可在设置中编辑名称和地址、增删服务商及模型。当前请求路径支持基础文本对话；Agent 文档上下文、工具调用和工作流尚未实现。

前端只显示 API Key 的星号遮罩，不返回具体 Key 给界面。遮罩只控制显示，不加密本机 JSON 文件；不得把真实 Key 写入 Git、ACR、日志或样例。

聊天模型选择器显示「服务商名称 + 模型名称」，请求使用服务商 ID、Base URL、Key 和模型 ID 路由。当前聊天框未提供 ChatGPT 思考强度选择。ChatGPT 授权请求由 `ai-verification` 提供，两种渠道复用同一套基础聊天入口和交互。登录 ChatGPT 后也可切换为优先使用 API 服务商。

## 当前本机配置格式

OpenRead 把配置写入用户应用数据目录中的 `providers.json`。首版使用以下结构，示例 Key 为占位值。

```json
{
  "version": 1,
  "providers": [
    {
      "id": "00000000-0000-4000-8000-000000000001",
      "name": "示例服务商",
      "baseUrl": "https://example.invalid/v1",
      "apiKey": "example-placeholder",
      "models": ["example-model"]
    }
  ]
}
```

### Base URL 格式

填写服务商提供的 API 根地址，例如 `https://api.openai.com/v1`。地址须包含 `http://` 或 `https://` 和主机，可含端口及路径。保留服务商要求的路径前缀和版本段（如 `/v1`）；当前聊天调用会在其后拼接 `/chat/completions`。地址不能含用户名、密码、查询参数或片段；保存时移除首尾空白和末尾斜杠。

设置使用 `list_providers`、`save_provider`、`delete_provider` 三个 Tauri 命令。读取和保存结果只返回 `id`、`name`、`baseUrl`、`models` 和 `hasKey`。新增时填写 Key，编辑时留空会保留现值。聊天用 `list_chat_models(source: "api")` 列出已配置且有 Key 的模型；返回的不透明模型路由 ID 只能映射到当前配置中的服务商与模型。请求向已保存的 Base URL 拼接 `/chat/completions`，API Key 不会返回前端。
