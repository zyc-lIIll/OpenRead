# API 服务商配置

`providers/` 保存用户配置的 API 服务商、API Key 和模型 ID。它负责本机配置与 Tauri 设置接口，不负责模型协议解析。所有协议转换归 [Agent model](../agent/model/README.md)。

## 配置接口

配置存放在用户应用数据目录的 `providers.json`。每个服务商有稳定 ID、可编辑名称、Base URL、API Key 和模型列表。设置调用 `list_providers`、`save_provider`、`delete_provider`。前端只收到 `id`、`name`、`baseUrl`、`models` 和 `hasKey`。编辑时留空 Key 保留已有值，Key 本身不会返回前端。

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

配置文件当前不加密。星号仅用于界面遮罩。真实 Key 不得进入 Git、日志、错误消息、样例或 ACR。

## 模型路由

`list_chat_models(source: "api")` 只列出已有 Key 的模型。前端选择器传回后端签发的不透明路由 ID，后端从保存的服务商 ID、模型 ID、Base URL 和 Key 解析请求。API Key 请求由 `agent/model/other.rs` 发往 Base URL 下的 `/chat/completions`，不提供通用联网搜索，也不使用 ChatGPT 专属思考强度。

API 服务商显示名称不决定协议。当前所有配置使用兼容 Chat Completions 的通用适配器。只有新协议需要稳定区分时，才增加适配器标识并处理旧配置兼容。

## Base URL 校验

Base URL 必须是带主机的 HTTP(S) 地址，可以含端口和服务商路径前缀。拒绝用户名、密码、查询参数和片段；保存时去掉首尾空格与末尾斜杠。请求只向保存配置和适配器规定的 endpoint 发送。

ChatGPT 登录和额度模型由 [ai-verification](../ai-verification/README.md) 与 `agent/model/chatgpt.rs` 负责。
