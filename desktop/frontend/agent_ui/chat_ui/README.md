# 共用 Chat UI

`chat_ui` 使用 OpenAI ChatKit React 显示阅读台主对话。后端当前接入 Explanation Agent；各对话 Agent 自己实现循环，共用显示事件。会话只在应用运行期间保留，不写入 ACR，也不接入 Annotation 浮窗。

- 模型列表按设置中的额度优先来源显示：ChatGPT 账户额度或 API 服务商额度。列表由本机后端提供，选中的模型 ID 随 ChatKit 请求交给 Rust 服务路由。
- 「设置 → 默认模型」分别指定 API 和 ChatGPT 渠道的默认模型；当前聊天仍可手动切换。Source 初始化也使用所选渠道的默认模型。
- ChatGPT 可在设置中选择默认思考强度，并在聊天框中单独调整；选项为自动、低、中、高。自动模式省略 API 参数，低、中、高通过本机请求头传给 Rust 后端，再映射为 Responses API 的 `reasoning.effort`。API Key 路由不显示也不发送该 ChatGPT 专属参数。
- ChatGPT 与 API 服务商共用同一个聊天入口和交互；来源只影响可选模型与请求路由。模型标签由后端提供，可呈现服务商与模型名称。
- 当前隐藏线程历史和附件入口；会话不持久化。界面会显示连接或模型加载状态、错误及重试入口。PDF 原文引用经 EntitySource 注释呈现为可点击位置，点击事件交给 Reader 激活 Source 并跳页。
- Agent 的过程文字、搜索进度和工具状态由 `client_effect` 临时显示，不写入助手消息或线程历史。正式正文分片追加到同一条助手消息；完成片段的引用按正文位置保存。Agent 切换显示去向时清除当前临时状态；取消或错误也会清理临时状态，并保留已流出的正式片段作为未完成内容。
- Explanation 建议另开对话时，界面显示“新开对话/暂不”确认。按钮通过已认证的 ChatKit `sendCustomAction` 调用后端；确认后新建空白线程草稿并预填当前对话最新用户原文，不会自动发送或复制旧对话历史。
- ChatKit 使用官方托管脚本和本地服务端点；前端从 Tauri 获取每次启动生成的访问令牌，并仅通过 Authorization 请求头发送给本地服务。
- Windows 正式打包前，发布方须通过 `VITE_CHATKIT_DOMAIN_KEY` 配置获发的 domain key，并在 OpenAI ChatKit 中登记打包应用的 Tauri 页面 origin。开发环境默认使用官方 quickstart 示例的 `local-dev`；打包后的 Tauri origin 尚待运行时验证和登记确认。

Agent 流程图和共用显示事件见 [`desktop/backend/agent/README.md`](../../../backend/agent/README.md)；Explanation 的循环与工作流选择位于 `desktop/backend/agent/explanation/`。共用工具注册和引用解析分别位于 `agent/tool/`、`agent/citation.rs`。当前 Annotation 工作流、附件、会话持久化和 ACR 写入仍未实现。

登录授权和 API Key 保存不属于这个 UI 模块。
