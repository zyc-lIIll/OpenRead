# API 服务商与模型

`providers` 管理用户配置的 API 服务商、API Key 和模型 ID。配置保存在本机 JSON 中，可同时包含多个服务商。每个服务商包含稳定 ID、名称、Base URL、API Key 和模型 ID 列表；用户可在设置中编辑名称和地址、增删服务商及模型。API 模型路由由 `agent/model/other.rs` 适配为通用 Chat Completions 请求，供 Explanation 和 PDF Source 初始化共同使用。该路由不提供通用联网搜索；适配器会明确告诉模型搜索不可用。供应商显示名称不用于猜测专属协议，也不需要额外的适配器标识。

前端只显示 API Key 的星号遮罩，不返回具体 Key 给界面。遮罩只控制显示，不加密本机 JSON 文件；不得把真实 Key 写入 Git、ACR、日志或样例。

聊天模型选择器显示「服务商名称 + 模型名称」，请求使用服务商 ID、Base URL、Key 和模型 ID 路由。ChatGPT 的默认思考强度及聊天框单次选择由 Agent/UI 负责；API Key 路由不传 ChatGPT 专属思考强度。ChatGPT 授权请求由 `ai-verification` 提供，并由独立的 `agent/model/chatgpt.rs` 处理。两种渠道复用同一套统一 Agent 请求和聊天入口；登录 ChatGPT 后也可切换为优先使用 API 服务商。

## 服务商协议适配

协议适配归 `agent/model/`，不放在本目录。当前 `other.rs` 将所有配置的 API 服务商按兼容 Chat Completions 的通用接口调用；`chatgpt.rs` 独立处理 ChatGPT 登录路由。只有现有模型路由不足以稳定识别新协议时，才扩展路由标识和兼容旧配置的迁移；不按用户可编辑的服务商名称猜协议，也不创建未实现的占位适配器。

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
