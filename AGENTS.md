# OpenRead 开发指南

## 项目目标与文档职责

OpenRead 是面向多学科阅读任务的 Windows 桌面 AI 阅读器。产品目标和当前状态以根目录 [README.md](README.md) 为准；模块 README 补充该模块的职责、实现状态和限制；本文件约定施工方式与代码放置。

`Plan.md` 已移除，不要恢复、引用或依赖它来记录项目进度。

## 当前实现基线

- 已有 Windows/Tauri 桌面框架、欢迎页与设置、PDF 导入/阅读/按页解析、Markdown 标题/章节/图片锚点解析、KaTeX 概览渲染、ChatKit，以及 ChatGPT 登录/额度授权和 API 服务商配置与模型路由。
- 已实现首版 PDF Source 初始化：结合 PDF 文本与页面图像，分段更新 Markdown 概览；ChatGPT 路由可按需联网搜索。概览保存在前端阅读器和后端 Workspace 内存，供 Explanation 工具按需读取；不持久化。
- 主对话 Explanation Agent 已实现五类工作流选择、学科阅读 skills、共享 Source 工具、模型响应统一适配、长回答工具循环、ChatGPT 路由的联网搜索以及可点击的 PDF 页引用。Annotation 问答/自动便签、Explanation 文档版本和会话持久化尚未实现。ChatGPT 思考强度可在设置中选择并由聊天框覆盖；API Key 路由不显示该选项。
- 当前仅支持 PDF 导入和原文阅读。Markdown 结构锚点解析已实现，但 Markdown 文件导入/阅读尚未实现；其他文件格式、ACR 应用读写和本地 OCR 尚未实现。扫描 PDF 可把页面图像交给支持视觉输入的模型。通用联网搜索仅由 ChatGPT Responses 路由提供，API Key 路由无搜索。除非任务明确要求，不要把这些功能顺手加入当前范围。

根 README 中标为“目标”或“草案”的内容不是已实现功能。修改文档时明确区分现状、产品约定和待讨论事项。

## 开始、施工与交付

- 开始前阅读根 README、本次涉及的模块 README 和相关实现；检查现有工作区改动，避免覆盖用户或其他施工者的修改。
- 施工前说明仓库/工作树路径、允许改动的目录或文件、职责边界及交付物；完成后报告实际改动位置。
- 对涉及核心交互、文件兼容性、凭据处理或技术栈的变更，先说明影响和实现选择，再按用户授权范围施工。
- 需要计划的实现任务应写清工作区域、文件所有权、交付物和从用户入口验收的条件。按当前协作约定由主代理施工、另一位 Sol 独立只读验收；Sol 报告发现和证据，不修改施工结果。若指定验收路由不可用，明确说明，不把施工者自检说成独立验收。
- 保留工作区中与当前任务无关的改动。不得提交、发布或向外部服务发送内容，除非用户明确要求。
- Windows 安装包以 `desktop/backend/tauri.conf.json` 中的 NSIS 配置为准；构建产物和测试输出放系统临时目录。未签名的本地测试包不代表正式发布。
- 交付说明写明操作方式、实际验证路径和仍待验收项。模拟响应只能证明应用流程；只有真实请求成功才能确认模型接入。

## 代码放置

- `desktop/frontend` 使用 TypeScript、React、CSS 和 Vite。应用框架、欢迎页和设置放 `app_ui`；格式化阅读器和交互放 `source/PDF`、`source/markdown` 等格式目录；通用原文阅读 UI 放 `source_ui`；讲解入口、批注浮窗和共用聊天组件分别放 `agent_ui/explanation_ui`、`agent_ui/annotation_ui`、`agent_ui/chat_ui`。
- `desktop/backend` 是 Rust/Tauri 本地后端。文档导入、解析和稳定锚点归 `source`；供应商请求/响应格式对接归 `agent/model`；共用工具归 `agent/tool`，按格式获取 Agent 上下文归 `agent/tool/source`；讲解和批注工作流分别归 `agent/explanation`、`agent/annotation`；学科共用 skills 归 `agent/skill`，Explanation 工作流 skills 归 `agent/explanation/skill`；批注锚点和关系归 `annotation`；ChatGPT 授权归 `ai-verification`；服务商配置归 `providers`；ACR 读写归 `acr`。
- 后端管理导入原件和文档数据，前端负责呈现与阅读交互。按格式选成熟引擎；同一引擎能提供预览和文本时优先复用已打开的文档。共用接口描述格式能力，定位结构按格式选择页码、章节或段落锚点。
- ACR 公约和兼容性样例放顶层 `ACR/`；应用测试和专用夹具放顶层 `tests/`；运行生成物放系统临时目录。

## 产品约定

- 首阶段聚焦 OpenAI API 和 ChatGPT 登录用户的阅读体验。Source 初始化和 Explanation 可在 ChatGPT 路由中按需联网搜索；Explanation 通过模型 API `tools` 参数注册本机 Source 工具。API Key 路由不提供通用联网搜索。暂不做本地 OCR；其他大型厂商和第三方服务商留待后续阶段。
- Source 在左，Explanation 在右。原文只读；讲解可按章节阅读，每次重新生成形成新版本，已有批注仍关联原版本。
- ChatGPT 账号最多登录一个。登录且额度授权成功后默认使用 ChatGPT；用户可切换为 API 服务商，未登录时使用已配置的服务商。渠道只影响可选模型和请求路由，不改变聊天交互。
- ChatGPT 思考强度默认值在设置中选择，聊天框可对后续请求单独调整；自动模式省略 `reasoning.effort`，低/中/高传入 Responses API。PDF Source 初始化使用默认值。API Key 模型不显示或接收 ChatGPT 专属强度选项；模型不接受明确强度时向用户报错，不自动改值。
- 服务商配置保存在本机 JSON 中，包含名称、Base URL、API Key 和模型；界面用星号遮住 Key，但 JSON 不加密。密钥不得进入 ACR、日志、样例或 Git。
- 欢迎页可选择 ChatGPT 登录或先进入阅读台并稍后配置 API 服务商。设置的账号页显示当前授权账户。
- 根 README 的产品行为优先于 `reference/approved-demo/`；Demo 仅作视觉参考。授权、模型路由和 Agent 业务不能绑定到某个视觉组件。
- Annotation 的交互类型为 `comment` 和 `question`，但用户不必预先选择类型。Agent 计划在讨论过程中自动整理便签；这和完整讨论记录是不同数据。
- ACR 目标包含 Source 原件、主对话、讲解/便签及批注完整讨论和关系、引用与附件；不得包含账号凭据或 API Key。读取时校验容器、版本、路径和引用，限制解包大小；写入失败要保留已有文件。

## 验收

从用户实际入口验证改动。记录运行系统、安装包和通过的路径；分别报告账号登录、额度授权和模型请求结果。ACR 功能实现后，使用同一份样例验证导出、干净环境重开和独立实现兼容性。
