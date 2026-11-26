# OpenRead 开发指南

## 项目目标与文档职责

OpenRead 是面向多学科阅读任务的 Windows 桌面 AI 阅读器。产品目标和当前状态以根目录 [README.md](README.md) 为准；模块 README 补充该模块的职责、实现状态和限制；本文件约定施工方式与代码放置。

### 模块文档索引

| 范围 | README |
| --- | --- |
| 产品与 ACR | [产品与实现状态](README.md) · [ACR 格式草案](ACR/README.md) · [ACR 后端](desktop/backend/acr/README.md) |
| 后端 Agent | [Agent 总览](desktop/backend/agent/README.md) · [模型请求与响应](desktop/backend/agent/model/README.md) · [Source 初始化](desktop/backend/agent/initial/README.md) · [Explanation](desktop/backend/agent/explanation/README.md) · [Explanation skills](desktop/backend/agent/explanation/skill/README.md) · [通用阅读 skills](desktop/backend/agent/skill/README.md) · [工具](desktop/backend/agent/tool/README.md) · [Source 上下文工具](desktop/backend/agent/tool/source/README.md) · [Annotation Agent（未实现）](desktop/backend/agent/annotation/README.md) |
| 后端服务 | [Source 导入与解析](desktop/backend/source/README.md) · [Markdown 锚点](desktop/backend/source/markdown/README.md) · [服务商配置](desktop/backend/providers/README.md) · [ChatGPT 授权](desktop/backend/ai-verification/README.md) |
| 前端 | [应用框架](desktop/frontend/app_ui/README.md) · [Chat UI](desktop/frontend/agent_ui/chat_ui/README.md) · [Explanation UI](desktop/frontend/agent_ui/explanation_ui/README.md) · [Annotation UI（未实现）](desktop/frontend/agent_ui/annotation_ui/README.md) · [Source UI](desktop/frontend/source_ui/README.md) · [Source 阅读器目录](desktop/frontend/source/README.md) · [PDF](desktop/frontend/source/PDF/README.md) · [Markdown/TeX](desktop/frontend/source/markdown/README.md) |

`Plan.md` 已移除，不要恢复、引用或依赖它来记录项目进度。

## 当前实现基线

- 已有 Windows/Tauri 桌面框架、欢迎页与设置、PDF 导入/阅读/按页解析、Markdown 标题/章节/图片锚点解析、KaTeX 概览渲染、ChatKit，以及 ChatGPT 登录/额度授权和 API 服务商配置与模型路由。
- 已实现首版 PDF Source 初始化：结合 PDF 文本与页面图像，分段更新 Markdown 概览；ChatGPT 路由可按需联网搜索。概览保存在前端阅读器和后端 Workspace 内存，供 Explanation 工具按需读取；不持久化。
- 主对话 Explanation Agent 已实现五类工作流选择、学科阅读 skills、共享 Source 工具、模型响应统一适配、长回答工具循环、ChatGPT 路由的联网搜索以及可点击的 PDF 页引用。Annotation 问答/自动便签、Explanation 文档版本和会话持久化尚未实现。ChatGPT 思考强度可在设置中选择并由聊天框覆盖；API Key 路由不显示该选项。
- 当前仅支持 PDF 导入和原文阅读。Markdown 结构锚点解析已实现，但 Markdown 文件导入/阅读尚未实现；其他文件格式、ACR 应用读写和本地 OCR 尚未实现。扫描 PDF 可把页面图像交给支持视觉输入的模型。通用联网搜索仅由 ChatGPT Responses 路由提供，API Key 路由无搜索。除非任务明确要求，不要把这些功能顺手加入当前范围。

根 README 中标为“目标”或“草案”的内容不是已实现功能。修改文档时明确区分现状、产品约定和待讨论事项。

## Agent 统一请求与响应框架

不同服务商的消息、图片、工具、流式响应、续传和错误协议不同。工作流不得分别拼接服务商请求 JSON、读取服务商 SSE 或解析服务商响应；统一边界如下：

- `initial/`、`explanation/` 及未来的 `annotation/` 只构造 OpenRead 请求：所选模型路由、顶层 `instructions`、有序 `input`、`tools`、`tool_choice`、`stream`、`store` 和适用的思考强度。
- `model/mod.rs` 是唯一中心入口，按路由选择适配器并将响应流归一后交回工作流。每个任务开始时解析并持有路由快照，避免模型列表刷新中断长任务；每次实际请求仍须由适配器重新检查当下的授权或服务商配置。
- `model/` 内每个已支持的服务商协议使用独立适配器文件；适配器同时转换请求和解析响应。当前 ChatGPT 登录使用 Responses，API Key 配置使用通用 Chat Completions。不得按可编辑的服务商显示名称猜协议，不创建空占位适配器；只有稳定路由信息不足时才扩展路由标识，并兼容旧配置。
- 适配器归一化文本增量、函数调用、搜索进度与引用、完成、截断、错误及适用的图像拒绝状态。Agent 根据统一结果显示文本，或把本机函数交给 `tool::dispatch` 执行，再将函数结果加入下一轮 `input`；也可结束或返回错误。
- 模型可用的函数必须在每次请求的 `tools` 中提供名称、说明和 JSON Schema。system prompt 中提到工具不等于向 API 注册了工具。`web_search` 表示搜索意图：适配器可映射到服务商能力；无实现时必须明确不可用，不得假称搜索已执行。无法提供的必需工具要报错，不可静默忽略。
- 服务商专属续传数据只能由模型层保存在不透明上下文中；Agent 只原样传递，不检查、不写入通用消息、ChatKit 或日志。`store: false` 只表示请求不使用该服务端对话存储能力，不承诺服务商没有服务日志。
- 等待网络、错误正文或响应流时，取消信号优先；取消后不得把未完成结果作为完成内容写入工作区。工作流可以处理自己的重试和进度规则，但不得重新实现协议解析。

请求字段、协议映射、事件形状和当前实现见[模型模块 README](desktop/backend/agent/model/README.md)。

## 开始、施工与交付

- 开始前阅读根 README、本次涉及的模块 README 和相关实现；检查现有工作区改动，避免覆盖用户或其他施工者的修改。
- 施工前说明仓库/工作树路径、允许改动的目录或文件、职责边界及交付物；完成后报告实际改动位置。
- 对涉及核心交互、文件兼容性、凭据处理或技术栈的变更，先说明影响和实现选择，再按用户授权范围施工。
- 需要计划的实现任务应写清工作区域、文件所有权、交付物和从用户入口验收的条件。按当前协作约定由主代理施工、另一位 Sol 独立只读验收；Sol 报告发现和证据，不修改施工结果。若指定验收路由不可用，明确说明，不把施工者自检说成独立验收。
- 保留工作区中与当前任务无关的改动。不得提交、发布或向外部服务发送内容，除非用户明确要求。
- Windows 安装包以 `desktop/backend/tauri.conf.json` 中的 NSIS 配置为准；构建产物和测试输出放系统临时目录。未签名的本地测试包不代表正式发布。
- Windows ChatKit 打包需按 [Chat UI 文档](desktop/frontend/agent_ui/chat_ui/README.md) 配置 domain key 并登记正式 origin；不能用开发 `local-dev` key 作为正式包验收证据。
- 交付说明写明操作方式、实际验证路径和仍待验收项。模拟响应只能证明应用流程；只有真实请求成功才能确认模型接入。

## 代码放置

- `desktop/frontend` 使用 TypeScript、React、CSS 和 Vite。应用框架、欢迎页和设置放 `app_ui`；格式化阅读器和交互放 `source/PDF`、`source/markdown` 等格式目录；通用原文阅读 UI 放 `source_ui`；讲解入口、批注浮窗和共用聊天组件分别放 `agent_ui/explanation_ui`、`agent_ui/annotation_ui`、`agent_ui/chat_ui`。
- `desktop/backend` 是 Rust/Tauri 本地后端。文档导入、解析和稳定锚点归 `source`；供应商请求/响应格式对接归 `agent/model`；共用工具归 `agent/tool`，按格式获取 Agent 上下文归 `agent/tool/source`；讲解和批注工作流分别归 `agent/explanation`、`agent/annotation`；学科共用 skills 归 `agent/skill`，Explanation 工作流 skills 归 `agent/explanation/skill`；批注锚点和关系归 `annotation`；ChatGPT 授权归 `ai-verification`；服务商配置归 `providers`；ACR 读写归 `acr`。
- 后端管理导入原件和文档数据，前端负责呈现与阅读交互。按格式选成熟引擎；同一引擎能提供预览和文本时优先复用已打开的文档。共用接口描述格式能力，定位结构按格式选择页码、章节或段落锚点。
- ACR 公约和兼容性样例放顶层 `ACR/`；应用测试和专用夹具放顶层 `tests/`；运行生成物放系统临时目录。

### 稳定锚点与呈现安全

- PDF 页序从 1 开始且包含封面。页内矩形以未旋转可见页面左上角为原点，按页面宽高归一化到 `0–1`；Source ID、页序与矩形共同标识位置。缩放、滚动、屏幕像素和 DPR 都不是持久锚点。
- Markdown 源范围使用从 0 开始、左闭右开的 UTF-8 字节偏移；章节范围按标题层级定义。Markdown 图片矩形由前端根据当前 DOM 和布局计算，不将 PDF 页码模型强加给 Markdown。
- Markdown/TeX 公式选择应识别完整公式并保留原始 TeX，不把公式拆成普通文字片段；该交互接入前仍属于目标约定。
- Markdown 渲染不执行原始 HTML；外部链接仅允许 HTTP(S)，并由后端打开。公式在界面中用 KaTeX 呈现，原始 Markdown/TeX 文本保持不变。
- PDF 页面图像最长边不超过 2,048 px、单张不超过 4 MiB；Source 初始化单段最多 12,000 个 Unicode 字符和 8 张图像；`read_source` 每次最多 5 页且工具输出不超过 48,000 字节。若调整这些边界，须同步更新对应模块 README、实现和测试。

### 凭据、路由与输入边界

- 前端只传后端签发的不透明模型路由 ID 和 Source ID；不得让前端传入任意 API URL、文件路径或凭据。后端按路由 ID 查询当前配置，并在 `Workspace` 中校验 Source UUID；工具不得以原始路径读取文件。
- API Key 和 OAuth token 只由 Rust 后端读取，前端仅获得必要状态（如 `hasKey`）和不透明路由 ID；不要在日志、错误消息、样例、ACR 或 Git 中暴露密钥。服务商 JSON 中的 API Key 当前不加密，星号只是界面遮罩。
- API Base URL 必须校验 HTTP(S) scheme 和主机，拒绝用户名、密码、查询参数和片段；保留服务商的路径前缀。请求只使用已保存配置和适配器规定的 endpoint。
- ChatGPT 会话凭据沿用 `ai-verification` 现有的 Windows DPAPI 保护和原子更新流程。临时网络或服务端错误应保留可重试凭据；只有确认令牌失效、切换账号或用户明确退出时才清除活动凭据。不要在 Agent 或 UI 中复制实现令牌存储/刷新。
- ChatKit 本机端点只绑定 loopback；每次启动生成的 Bearer token 只经 Authorization 头交给本机服务，不进入日志。Windows 正式包必须配置 `VITE_CHATKIT_DOMAIN_KEY` 并登记实际 Tauri origin；开发 `local-dev` key 不代表打包 origin 已验收。
- Agent 输入中的原文、概览和模型输出都是不可信数据。工具名、JSON 参数、Source ID、页码范围和结果大小必须在后端校验；提示词约束不能代替边界校验。

## 产品约定

- 首阶段聚焦 OpenAI API 和 ChatGPT 登录用户的阅读体验。Source 初始化和 Explanation 可在 ChatGPT 路由中按需联网搜索；Explanation 通过模型 API `tools` 参数注册本机 Source 工具。API Key 路由不提供通用联网搜索。暂不做本地 OCR；其他大型厂商和第三方服务商留待后续阶段。
- Source 在左，Explanation 在右。原文只读；讲解可按章节阅读，每次重新生成形成新版本，已有批注仍关联原版本。
- ChatGPT 账号最多登录一个。登录且额度授权成功后默认使用 ChatGPT；用户可切换为 API 服务商，未登录时使用已配置的服务商。渠道只影响可选模型和请求路由，不改变聊天交互。
- ChatGPT 思考强度默认值在设置中选择，聊天框可对后续请求单独调整；自动模式省略 `reasoning.effort`，低/中/高传入 Responses API。PDF Source 初始化使用默认值。API Key 模型不显示或接收 ChatGPT 专属强度选项；模型不接受明确强度时向用户报错，不自动改值。
- 服务商配置保存在本机 JSON 中，包含名称、Base URL、API Key 和模型；界面用星号遮住 Key，但 JSON 不加密。密钥不得进入 ACR、日志、样例或 Git。
- 欢迎页可选择 ChatGPT 登录或先进入阅读台并稍后配置 API 服务商。设置的账号页显示当前授权账户。
- 根 README 的产品行为优先于 `reference/approved-demo/`；Demo 仅作视觉参考。授权、模型路由和 Agent 业务不能绑定到某个视觉组件。
- Source 标签移除只从工作区移除来源，不删除导入原件；切换 PDF 标签须保留各文档的页码和缩放状态。阅读台具体交互、比例范围和键盘行为见 [app_ui README](desktop/frontend/app_ui/README.md)。
- Annotation 的交互类型为 `comment` 和 `question`，但用户不必预先选择类型。Agent 计划在讨论过程中自动整理便签；这和完整讨论记录是不同数据。
- Annotation 使用共用浮窗聊天；定位锚点与浮窗屏幕位置分开保存，用户可关闭讨论后恢复，也可在回复上继续追问。便签必须能关联回完整 Annotation 讨论；具体目标上下文见根 README 和 [Annotation UI README](desktop/frontend/agent_ui/annotation_ui/README.md)。
- ACR 目标包含 Source 原件、主对话、讲解/便签及批注完整讨论和关系、引用与附件；不得包含账号凭据或 API Key。读取时校验容器、版本、路径和引用，限制解包大小；写入失败要保留已有文件。

## 验收

从用户实际入口验证改动。记录运行系统、安装包和通过的路径；分别报告账号登录、额度授权和模型请求结果。ACR 功能实现后，使用同一份样例验证导出、干净环境重开和独立实现兼容性。
