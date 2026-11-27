# OpenRead 开发指南

## 开工阅读

- 开始前阅读根目录 [README.md](README.md)，理解产品目标、当前实现和整体架构。
- 阅读本次涉及的模块 README 和实现入口。README 说明模块职责和公约，代码与测试是实现状态的依据。
- 检查工作区改动。保留用户及其他施工者的变更，避免覆盖或清理无关文件。
- `Plan.md` 已移除，不要恢复或依赖它记录进度。

根 README 和模块 README 将现状、产品目标及草案分别标注。没有明确任务时，不把未实现格式、OCR、ACR 或 Annotation 功能顺手加入当前范围。

## 文档编写与架构记录

- 根目录 `README.md` 是项目级产品行为、整体架构和实现状态的依据：说明产品目标、ACR 公约与草案、目录职责、技术栈及文档导航。明确区分“已实现”“目标”“草案”和“待讨论”，不得把规划写成现成功能。
- 每个模块 README 说明本目录的目的、统一/对外接口及边界、文件和子目录职责、当前使用者、实现状态与限制。按“本目录做什么、如何被调用、哪些子目录各自负责什么”组织；模块细节放在模块文档，根 README 保持总览。
- 长 prompt 写在 Markdown 文档中；在实际调用它的程序里明确写出拼接了哪些文档。短的固定 prompt 直接留在调用程序中，按重要程度使用常量或行间字符串，不另建只负责拼接的小模块。
- 同一公约按项目总览和模块细节分层说明。根 README 放模型请求/响应总览流程图和抽象 Agent 双阶段显示流程图；`desktop/backend/agent/model/README.md` 是模型请求字段、协议映射、事件及模型流程图的详细规范；`desktop/backend/agent/README.md` 放共享组件、组件使用位置和共用显示事件；`desktop/backend/agent/explanation/README.md` 放 Explanation 的实际流程图及专属控制规则。其他文档链接到对应详细规范，不复制整段细节或相同流程图。
- 抽象公约与具体 Agent 实现分开描述。Explanation 图是 Explanation 实现示例，不代表共用显示公约只适用于 Explanation；`select_workflow`、`organize_output`、`continue_output`、`continue_collection` 等具体名称和行为仍标为 Explanation 专属。注明某项公约适用于谁，并另列当前实际使用者或未来使用者。
- 描述共用接口时先说明稳定契约和职责边界，再清楚列出“谁调用了什么、目前在哪里使用”。不要把抽象流程、具体实现、使用位置和实现状态混写在同一段里。
- 提出结构合并建议前先查调用点和实现。优先检查只有单一调用方的转发包装、纯中转文件、重复参数转换或重复职责；给出实际证据、候选合并位置和需保留的校验/错误行为。不要仅因文件较小就建议删除有完整状态或领域职责的模块。建议必须标注为未实施；文档任务不顺带修改实现代码。
- 文档应描述实际接口和代码行为，不以提示词内容代替接口说明。实现、状态或边界变化时，更新其权威模块 README，并同步修正根 README 的总览和导航；避免无关文档重复维护同一细节。

## 项目架构与代码放置

OpenRead 是面向多学科阅读任务的 Windows 桌面应用。前端负责呈现和阅读交互，本地 Rust 后端负责原件管理、解析、模型路由、Agent 工作流和授权。产品行为和模块导航见 [README](README.md)。

| 目录 | 职责 |
| --- | --- |
| `desktop/frontend/app_ui/` | 欢迎页、设置、阅读台和页面级状态 |
| `desktop/frontend/agent_ui/chat_ui/` | Explanation 与未来 Annotation 共用的聊天呈现 |
| `desktop/frontend/agent_ui/explanation_ui/`、`annotation_ui/` | 各自 Agent 的前端入口与交互 |
| `desktop/frontend/source/`、`source_ui/` | 按格式呈现原文、阅读交互和通用概览 |
| `desktop/backend/source/` | 导入原件、格式解析和稳定锚点 |
| `desktop/backend/agent/model/` | 统一请求、服务商协议转换和流式响应归一 |
| `desktop/backend/agent/initial/` | Source 概览初始化流水线 |
| `desktop/backend/agent/explanation/`、`annotation/` | 各自的对话 Agent loop 和工作流。Annotation 尚未实现 |
| `desktop/backend/agent/tool/`、`skill/` | 根目录保存各 Agent 共用工具和 skill；Agent 专属内容分别放入 `tool/<agent>/` 和 `skill/<agent>/` |
| `desktop/backend/agent/tool/markdown.rs` | 按编译期白名单读取内置 Markdown 指引，不接收任意文件路径 |
| `desktop/backend/agent/mod.rs`、`chatkit.rs`、`citation.rs` | 共用路由与显示事件、本机 ChatKit 服务、流式引用解析 |
| `desktop/backend/providers/`、`ai-verification/` | API 服务商配置、ChatGPT 授权和凭据 |
| `ACR/`、`desktop/backend/acr/` | ACR 格式草案与未来读写实现 |

Source 的职责分布在后端导入解析、前端阅读呈现、Agent 上下文读取三处。后端解析产生的原文和锚点由前端呈现，Agent 工具按需取用，不从 UI 或模型参数读取任意文件路径。格式能力及子模块详见各自 README。

## Agent 公约

- `desktop/backend/agent/model/README.md` 定义统一请求与响应。Source 初始化和对话 Agent 构造 OpenRead 请求，模型层按不透明路由 ID 选择适配器、解析响应流；Agent 管理业务循环并执行本机函数。
- `desktop/backend/agent/README.md` 定义对话 Agent 共用显示事件。新回复从阶段 1 开始，取材期间通过 `Activity` 临时显示可见过程文字和实际工具状态；阶段 2 清理当前片段播报，并以 `Delta` 流式写入同一条助手消息。阶段 2 可以正常结束、续写当前片段，或完成当前片段后回阶段 1 取材。
- 各 Agent 自己负责循环、工具白名单、上下文策略、首轮规则、阶段控制、取消和错误处理。Explanation 的 `select_workflow`、`organize_output`、`continue_output` 与 `continue_collection` 都是 Explanation 专用行为。Annotation 遵循共用显示约定，但有自己的循环和业务规则。
- `agent/tool/` 和 `agent/skill/` 根目录只放共用实现或内容；专属项放在对应 Agent 子目录。新建 Annotation 时，在 `agent/annotation/` 独立实现 loop，在 `agent/tool/annotation/` 与 `agent/skill/annotation/` 放专属工具和长指引。
- 各对话 Agent 按 `model/README.md` 构造统一模型请求并消费归一化响应，按 `agent/README.md` 使用共用双阶段显示事件；每个 Agent 自己决定工具、阶段及回复何时结束。
- `CompletionEvent` 是对话 Agent 共用的显示接口。`Activity` 用于临时播报；正文、引用、片段完成、用户操作、完成和错误使用各自事件。ChatKit/UI 决定如何呈现。
- `agent/citation.rs` 处理跨 chunk 引用标记、校验目标和正文位置。它不负责 Agent loop。
- Source 初始化是独立流水线，继续使用自己的进度和结果接口，不接入对话 Agent 显示事件。
- 工具注册项绑定模型 schema 和 handler。各 Agent 只开放本轮工具子集，后端使用同一子集限制 dispatch。参数、Source ID、页码和结果大小必须在 Rust 后端校验。
- Responses 专属续传数据由模型层保存为不透明 `Context`。Agent 只原样传递，不放入通用显示事件、ChatKit 或日志。

## 稳定锚点和呈现安全

- PDF 页码从 1 开始并包含封面。页内矩形以未旋转可见页面左上角为原点，按宽高归一化到 `0–1`。Source ID、页码与矩形共同定位；缩放、滚动、屏幕像素和 DPR 不作为持久锚点。
- Markdown 源范围是从 0 开始、左闭右开的 UTF-8 字节偏移。章节范围按标题层级定义。Markdown 图片的可见矩形由前端依据 DOM 和当前布局计算。
- Markdown 渲染不执行原始 HTML。外部链接只允许 HTTP(S)，并通过后端打开。公式在界面中由 KaTeX 呈现，源 Markdown/TeX 保持原文。
- PDF 页图最长边不超过 2,048 px，单张不超过 4 MiB。Source 初始化片段最多 12,000 个 Unicode 字符和 8 张图像。`read_source` 每次最多 5 页，工具输出不超过 48,000 字节。调整这些边界时同步更新对应 README、实现和测试。

## 凭据、路由与输入边界

- Agent 模型请求和 Source 访问只接收后端签发的不透明模型路由 ID 与 Source ID，不接受任意模型 URL、文件路径或调用方凭据。设置页可通过 `save_provider` 提交用户新输入的 Base URL 和 API Key；后端校验并保存配置，读取结果只返回 `hasKey`。Agent 后续按不透明路由 ID 查询已保存配置，并在 `Workspace` 校验 Source UUID。
- 模型调用时，已保存的 API Key 与 OAuth token 只由 Rust 后端读取，不写入日志、错误、样例、ACR 或 Git。设置表单只在用户保存时提交新输入的 API Key。API 服务商 JSON 当前不加密，界面星号只是遮罩。
- API Base URL 必须校验 HTTP(S) scheme 和主机，拒绝用户名、密码、查询参数和片段；保留服务商路径前缀。请求仅使用已保存配置和适配器规定的 endpoint。
- ChatGPT 会话凭据沿用 `ai-verification` 的 Windows DPAPI 保护和原子更新流程。临时网络或服务端错误须保留可重试凭据；只有确认令牌失效、切换账号或用户明确退出时才清除活动凭据。
- ChatKit 本机服务只绑定 loopback。每次启动生成的 Bearer token 只经 Authorization 头传递，不进入日志。Windows 正式包需配置 `VITE_CHATKIT_DOMAIN_KEY` 并在 ChatKit 登记实际 Tauri origin；开发 `local-dev` key 不代表打包环境已验收。
- 原文、概览和模型输出都是不可信数据。提示词要求不能代替后端参数及数据边界校验。

## 协作、施工与交付

- 施工前说明仓库/工作树路径、允许改动的文件或目录、职责边界和交付物。核心交互、文件兼容性、凭据或技术栈变更要先说明影响和实现选择。
- 计划型实现由主代理施工，另一位 Sol 只读独立验收。Sol 报告发现和证据，不修改施工结果；路由不可用时如实说明，不能把施工者自检写成独立验收。
- 完成后报告实际改动位置、用户入口和验证路径，以及仍待验收项。不得提交、发布或向外部服务发送内容，除非用户明确要求。
- Windows 安装包以 `desktop/backend/tauri.conf.json` 中的 NSIS 配置为准。构建产物和测试输出放系统临时目录。未签名本地包不代表正式发布。
- 验收从用户实际入口进行，并记录运行系统、安装包和通过路径。账号登录、额度授权、模型请求分别报告。ACR 实现后，用同一份样例验证导出、干净环境重开和独立实现兼容性。

## 当前实现基线

- 已实现 Windows/Tauri 应用框架、欢迎页与设置、PDF 导入阅读和按页解析、Markdown 标题/章节/图片锚点解析、概览 KaTeX 渲染、ChatKit、本机授权及 API 服务商配置。
- 已实现 PDF Source 初始化和 Explanation 五类工作流。Explanation 有自己的双阶段 Agent loop、注册工具、ChatGPT 路由搜索和 PDF 页引用。对话 Agent 共用显示事件和引用解析。
- 当前只有 PDF 可导入阅读。Markdown 文件阅读、其他文件格式、Annotation Agent/UI、Explanation 文档版本、会话持久化、ACR 读写和本地 OCR 尚未实现。
- API Key 路由目前没有通用搜索实现。真实账号授权、模型请求和 Windows 安装包的当前验收状态以根 README 中的状态表和对应模块 README 为准。
