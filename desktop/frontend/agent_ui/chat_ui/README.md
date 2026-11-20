# 共用 Chat UI

`chat_ui` 使用 OpenAI ChatKit React 显示基础聊天界面，当前放在阅读台右侧；它尚未成为 Explanation Agent，也没有接入 Annotation 浮窗。当前版本通过本机 Tauri 服务连接 ChatKit；会话只在应用运行期间保留，不写入 ACR，也不关联当前文档。

- 模型列表按设置中的额度优先来源显示：ChatGPT 账户额度或 API 服务商额度。列表由本机后端提供，选中的模型 ID 随 ChatKit 请求交给 Rust 服务路由。
- 当前可以选择模型，但不能选择 ChatGPT 思考强度；UI 和后端请求均未实现该参数。
- ChatGPT 与 API 服务商共用同一个聊天入口和交互；来源只影响可选模型与请求路由。模型标签由后端提供，可呈现服务商与模型名称。
- 当前隐藏线程历史和附件入口；会话不持久化。界面会显示连接或模型加载状态、错误及重试入口。
- ChatKit 使用官方托管脚本和本地服务端点；前端从 Tauri 获取每次启动生成的访问令牌，并仅通过 Authorization 请求头发送给本地服务。
- Windows 正式打包前，发布方须通过 `VITE_CHATKIT_DOMAIN_KEY` 配置获发的 domain key，并在 OpenAI ChatKit 中登记打包应用的 Tauri 页面 origin。开发环境默认使用官方 quickstart 示例的 `local-dev`；打包后的 Tauri origin 尚待运行时验证和登记确认。

当前没有 Agent 提示词/上下文处理、文档阅读、工具调用、Explanation 生成或 Annotation 工作流。联网/图片搜索、附件和文档上下文也尚未实现。

登录授权和 API Key 保存不属于这个 UI 模块。
