# ChatGPT 授权

`ai-verification/ChatGPT` 负责 ChatGPT 登录、额度授权状态、刷新和退出；`agent` 使用后端授权访问模型目录和请求额度模型。登录使用 OpenAI Sign in with ChatGPT 公共客户端流程：在 `127.0.0.1` 启动 `/auth/callback` 回调，再打开系统浏览器。首次注册使用 `dynamic_agent_client`，保存回调返回的签发 client ID；后续授权复用它。流程校验随机 `state`、PKCE 和已签名的 OIDC ID token，并以令牌响应的 scope 判断是否有 `chatgpt.tokens.use.direct` 权限。

本模块只负责账号授权和额度模型访问；授权成功不代表 Explanation、Annotation 或其他 Agent 工作流已经实现。

账号会话写入用户应用数据目录的 `chatgpt/sessions.dat`，由当前 Windows 用户的 DPAPI 保护；更新使用原子替换，并以本机文件锁串行化登录提交、刷新和退出，避免多个进程同时轮换 refresh token。稳定 host ID 与签发 client ID 随本地注册信息保存。可保留多个注册的账号标识，但任一时刻只有当前账号的凭据保持有效；切换账号时会清除旧账号的本地令牌。未获额度权限的有效登录仍显示账号，但 `planGranted` 为 `false`。当前实现只支持 Windows 桌面凭据保护。

设置页通过以下 Tauri 命令读取和操作账号：

- `chatgpt_account_status` 返回 `state`（`signedOut`、`signedIn`、`reauthRequired`）、`account`、保存的 `registrations` 和 `signInPending`。账号对象包含 `registrationId`、`name`、`email` 和 `planGranted`，不会返回令牌。
- `chatgpt_sign_in` 接受 `registrationId` 和 `requestPlanConsent`。普通登录不强制弹出同意页；用户明确启用额度时，必须指定当前账号，并请求完整权限后再次核验 scope。
- `chatgpt_cancel_sign_in` 取消正在等待回调的登录；登录命令退出后 `signInPending` 清零。
- `chatgpt_sign_out` 先尝试撤销 refresh token，再清除本地活动凭据；返回的 `remoteRevoked` 表示远端撤销是否确认。无 refresh token 时本地仍会退出，但该字段为 `false`。

账号状态读取会在 access token 临近过期时尝试刷新。临时网络或服务端错误保留现有凭据供重试；确认不可用的 refresh token 会要求重新授权。首次授权码交换遇到 `invalid_grant` 时，界面可让用户重新发起登录。`list_chat_models(source: "chatgpt")` 在后端检查额度 scope 并从 `/v1/models` 读取当前账号可见模型；消息通过 `/v1/responses` 以 `store:false`、`stream:true` 请求，并在 `response.completed` 后才报告完成。令牌只由 Rust 后端读取，模型名和不透明路由 ID 才返回前端。真实授权和模型额度调用仍需在用户登录环境中验证。
