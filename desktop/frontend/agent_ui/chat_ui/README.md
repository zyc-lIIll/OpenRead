# 共用 Chat UI

`agent_ui/chat_ui/` 使用 ChatKit React 呈现 Agent 对话，管理模型选择、输入、流式消息和确认操作。每个对话 Agent 管理自己的 loop；前端按共享的 ChatKit 事件契约呈现内容。

## 文件职责和接口

- `ChatKitChat.tsx` 创建 ChatKit React 入口，取得本机服务地址和每次启动生成的 Bearer token，传递选中模型路由 ID和 ChatGPT 思考强度。请求只向 Rust 本机 ChatKit 服务发送。
- `chatkit.css` 设置 ChatKit 容器样式和临时 Activity 呈现。
- 后端事件及双阶段显示约定见 [Agent README](../../../backend/agent/README.md)，本机协议和凭据边界见后端 `agent/chatkit.rs`。

ChatGPT 与 API 服务商共用同一聊天入口。模型列表和不透明路由 ID 由本机后端提供。Source 初始化使用各渠道默认模型，不属于聊天组件的对话流程。

## 当前交互

- ChatGPT 默认思考强度可在设置中选择，也可在聊天框调整。自动模式不发送 `reasoning.effort`；API Key 路由隐藏此选项。
- `Activity` 用于阶段 1 的公开取材文字、搜索进度和实际工具状态，不写进助手正文或线程历史。阶段 2 的 `Delta` 更新同一条助手消息；每个完成片段的引用按正文位置附加。
- Agent 清理 Activity 后可以进入正式输出，也可以在下一片段取材时再次显示 Activity。取消或错误会清理临时内容，并保留已经输出的正式文字与完成引用。
- PDF EntitySource 引用点击后交给 Reader 激活对应 Source 并跳转页码。
- Explanation 可向用户提供“新开对话”确认。确认后把当前线程最新用户原文预填到新草稿，不复制历史、不自动发送。
- 当前线程只保存在运行期内存，附件和 Annotation 浮窗尚未接入。

## ChatKit 部署

Windows 正式包需设置获发的 `VITE_CHATKIT_DOMAIN_KEY`，并在 ChatKit 登记实际打包应用的 Tauri origin。开发环境的 `local-dev` key 只用于本机开发，不证明打包环境已验证。
