# 共用 Chat UI

`chat_ui` 使用 OpenAI ChatKit React 显示阅读台主对话。后端接入 Explanation Agent，可调用 Source、概览和 ChatGPT 联网搜索工具；会话只在应用运行期间保留，不写入 ACR，也不接入 Annotation 浮窗。

- 模型列表按设置中的额度优先来源显示：ChatGPT 账户额度或 API 服务商额度。列表由本机后端提供，选中的模型 ID 随 ChatKit 请求交给 Rust 服务路由。
- 「设置 → 默认模型」分别指定 API 和 ChatGPT 渠道的默认模型；当前聊天仍可手动切换。Source 初始化也使用所选渠道的默认模型。
- ChatGPT 可在设置中选择默认思考强度，并在聊天框中单独调整；选项为自动、低、中、高。自动模式省略 API 参数，低、中、高通过本机请求头传给 Rust 后端，再映射为 Responses API 的 `reasoning.effort`。API Key 路由不显示也不发送该 ChatGPT 专属参数。
- ChatGPT 与 API 服务商共用同一个聊天入口和交互；来源只影响可选模型与请求路由。模型标签由后端提供，可呈现服务商与模型名称。
- 当前隐藏线程历史和附件入口；会话不持久化。界面会显示连接或模型加载状态、错误及重试入口。PDF 原文引用经 EntitySource 注释呈现为可点击位置，点击事件交给 Reader 激活 Source 并跳页。
- ChatKit 使用官方托管脚本和本地服务端点；前端从 Tauri 获取每次启动生成的访问令牌，并仅通过 Authorization 请求头发送给本地服务。
- Windows 正式打包前，发布方须通过 `VITE_CHATKIT_DOMAIN_KEY` 配置获发的 domain key，并在 OpenAI ChatKit 中登记打包应用的 Tauri 页面 origin。开发环境默认使用官方 quickstart 示例的 `local-dev`；打包后的 Tauri origin 尚待运行时验证和登记确认。

System prompt、工作流分类、工具执行和引用校验位于 `desktop/backend/agent/explanation/` 与 `desktop/backend/agent/tool/`。当前 Annotation 工作流、附件、会话持久化和 ACR 写入仍未实现。

登录授权和 API Key 保存不属于这个 UI 模块。
