# ChatGPT 登录与额度授权

`ai-verification/` 负责 ChatGPT 登录、额度授权状态、token 刷新和退出。它提供凭据访问能力给 Agent，不控制 Explanation、Annotation 或任何模型工作流。

## 目录与授权流程

`ChatGPT/mod.rs` 实现 OAuth / OIDC、回调监听、token 生命周期和本机凭据存储；父级 `mod.rs` 暴露 `ChatGptService`。登录在 loopback 地址等待 `/auth/callback`，再打开系统浏览器。首次注册使用 `dynamic_agent_client` 并保存签发的 client ID；后续授权复用该注册。

授权过程校验随机 `state`、PKCE 和已签名的 OIDC ID token。额度权限根据授权 token 中的 `chatgpt.tokens.use.direct` scope 判断。有效登录可以没有额度授权，此时账号仍显示为已登录，但 `planGranted` 为 `false`。

## 前端接口

- `chatgpt_account_status` 返回登录状态、当前账号、已保存账号注册信息和登录等待状态，不返回 token。
- `chatgpt_sign_in` 接受注册 ID 与是否请求额度授权。额度授权需要针对当前账号明确请求并在返回后再次核验 scope。
- `chatgpt_cancel_sign_in` 取消正在等待回调的登录。
- `chatgpt_sign_out` 尝试撤销 refresh token 后清除当前本机凭据，并返回远端撤销是否确认。

设置页通过这些 Tauri 命令管理账号。`agent` 读取授权状态并查询当前账号能访问的 `/v1/models`。实际模型请求由 `agent/model/chatgpt.rs` 发送到 Responses API，使用 `store:false` 和流式响应，并等待 `response.completed`。

## 凭据保护与失败处理

账号会话存入用户应用数据目录 `chatgpt/sessions.dat`。当前 Windows 桌面使用 DPAPI 保护 token，通过原子替换和本机文件锁串行化登录提交、刷新和退出。只保留当前账号的活动凭据。切换账号会清除旧账号 token。

临时网络或服务端错误时保留凭据供重试；确认 refresh token 无效时要求重新授权。凭据只由 Rust 后端读取，不进入前端、日志或 ACR。当前凭据保护只支持 Windows，真实登录与额度请求仍需在用户登录环境验证。
