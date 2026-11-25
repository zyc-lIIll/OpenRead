# API 服务商与模型

`providers` 管理用户配置的 API 服务商、API Key 和模型 ID。配置保存在本机 JSON 中，可同时包含多个服务商。每个服务商包含稳定 ID、名称、Base URL、API Key 和模型 ID 列表；用户可在设置中编辑名称和地址、增删服务商及模型。普通 ChatKit 聊天目前只支持基础文本请求；PDF Source 初始化的文档上下文、图像输入和可选搜索由 `agent/initial/` 处理，并从本模块读取 API 服务商配置。Explanation、Annotation 和通用 Agent 工具尚未实现。

前端只显示 API Key 的星号遮罩，不返回具体 Key 给界面。遮罩只控制显示，不加密本机 JSON 文件；不得把真实 Key 写入 Git、ACR、日志或样例。

聊天模型选择器显示「服务商名称 + 模型名称」，请求使用服务商 ID、Base URL、Key 和模型 ID 路由。ChatGPT 的默认思考强度及聊天框单次选择由 Agent/UI 负责；API Key 路由不传 ChatGPT 专属思考强度。ChatGPT 授权请求由 `ai-verification` 提供，两种渠道复用同一套基础聊天入口和交互。登录 ChatGPT 后也可切换为优先使用 API 服务商。

## 服务商专属适配（后续规划）

当各大厂商需要专属的认证、模型目录、请求字段或响应处理时，在本目录按厂商拆分适配文件，不把所有差异堆进通用路由。规划的文件包括：

- `openai.rs`：OpenAI API Key 与模型适配。
- `glm.rs`：智谱 GLM API Key 与模型适配。
- `deepseek.rs`：DeepSeek API Key 与模型适配。
- `claude.rs`：Anthropic Claude API Key 与模型适配。

这些模块目前都未实现，也不创建占位文件。当前统一的 Base URL、Key 和模型配置继续由本 README 描述；厂商专属文件只在对应适配开始实现时加入。

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
