# OpenRead

OpenRead 是一款以文档为单位的 AI 辅助阅读器，最终目标是 Windows 桌面应用。用户把教材、课件或笔记放进同一个工作区，左侧阅读原文，右侧阅读讲解；遇到疑问时，从原文或讲解打开浮窗继续讨论。

不仅局限于单一学科。可以跨度各种学科、各种阅读任务。

当前阶段优先把 OpenAI API 和 ChatGPT 登录渠道的阅读体验做好。联网搜索目前仅用于 Source 初始化中的可选背景补充；图片搜索、本地 OCR，以及更多大型厂商和第三方服务商的适配留待后续阶段。PDF 扫描页可作为页面图像交给支持视觉输入的模型识读。

## 当前实现状态

| 功能 | 当前状态 |
| --- | --- |
| 桌面 UI | 已有基本应用框架、欢迎/设置入口和 Source/Explanation 双栏框架；当前右侧承载基础聊天，不是讲解文档界面。 |
| PDF | 已实现 PDF 导入、阅读、按页文字解析，以及供 Source 初始化使用的页面 PNG；PDF.js 也提供选区、截图及定位接口。 |
| Markdown 结构 | 已能从 Markdown 文字解析分级标题、章节范围及图片的源文本锚点；Markdown 文件导入和专属阅读界面尚未实现。 |
| 模型与账号 | 已实现 API 服务商配置、模型列表/路由，以及 ChatGPT 登录和额度授权流程；普通聊天支持基础文本对话，线程仅在应用运行时保留，不关联文档、不写入 ACR。真实账号授权和模型请求仍待 Windows 安装环境实测。 |
| 思考强度 | ChatGPT 默认强度可在设置中选择，聊天框可单次调整；自动、低、中、高已接入 ChatGPT 请求。API Key 模型不提供此选项。真实模型是否接受所选值仍待实测。 |
| Agent 功能 | 已实现 PDF Source 初始化和临时 Markdown 概览查看，包含页图、页码建议及 ChatGPT 路由的可选搜索；概览尚未接入后续聊天上下文，也不持久化。普通聊天的文档上下文组装、Explanation 生成、Annotation 问答/便签和会话保存仍未实现。基础模型对话不等于完整 Agent 工作流。 |
| 其他格式与 ACR | 目标格式还包括 Markdown、TXT、EPUB、DOC/DOCX、PPT/PPTX；当前应用仅支持 PDF。ACR 仍是格式设计，应用读写尚未实现。 |
| Windows 安装包 | 已从当前工作区生成 0.1.0 x64 未签名 NSIS 本地验收包，等待安装实测；正式版本仍计划经 GitHub Releases 分发。 |

下文标记为“产品目标”或“草案”的内容用于描述计划行为，不代表当前已经可用。

## 阅读工作区：产品目标

- **Source** 保存并只读呈现原始文件。可以导入多份材料并用标签切换。目标格式包括 PDF、Markdown、TXT、EPUB、DOC/DOCX 和 PPT/PPTX。
- **Explanation** 是独立的讲解文档，可生成概览、详细讲解或自定义内容。每次生成都建立新版本；讲解可按章节阅读，并通过引用回到原文。
- **Annotation** 是关联到原文、某版讲解或已有回复的评论与问题。关闭浮窗后可从摘要恢复讨论，也可以在回复上继续追问。

原文保持只读。重新生成讲解不会覆盖旧版本，已有批注仍指向创建时关联的原文或讲解版本。

## 聊天入口与模型路由

产品目标是让 Explanation 主对话和 Annotation 浮窗共用同一套聊天交互。当前只有阅读台右侧的基础聊天界面；尚无 Explanation 或 Annotation Agent 工作流。用户选择的 ChatGPT 账户或 API 服务商决定模型请求走哪条渠道，不改变聊天界面。

- 最多同时登录一个 ChatGPT 账号。设置的「账号」页显示当前账号、额度授权状态和模型来源开关：开关打开时只显示 ChatGPT 模型，关闭时只显示已配置的 API 服务商模型。开关不会退出账号；登录且额度授权成功后默认使用 ChatGPT，用户切换到 API 后会记住选择。未来增加其他登录来源时，同一时刻最多只能选择一个登录来源。
- 未登录时，使用用户配置的服务商和模型。欢迎页选择「使用 API Key」可先进入阅读台，之后再配置服务商。
- 模型选择器显示「服务商名称 + 模型」；选择结果决定实际请求路由。当前已有 ChatKit React 界面和 Rust 本机模型请求服务，可进行基础文本对话；线程只保存在运行期间，不关联 Source，也不写入 ACR。ChatGPT 的思考强度默认值在设置中配置，聊天框可针对当前会话调整；API Key 模型不显示此选项。通用联网/图片搜索、附件、文档上下文和完整 Agent 工作流均未接入。
- 服务商配置保存在本机 JSON 中，包含名称、Base URL、API Key 和模型。界面用星号遮住 API Key；遮罩只影响显示，不加密 JSON 文件。密钥不得写入 ACR、日志、样例或 Git。

## 批注与 PDF 选区：产品目标

产品只使用一种浮窗聊天，随入口附上不同上下文：

| 入口 | 带入聊天的内容 | 批注打开时的定位 |
| --- | --- | --- |
| 选中文字或整条公式 | 引用文字 | 原文中的对应选区 |
| 对 PDF 或 PPT 当前页提问 | 当前页内容 | 对应原文页 |
| 对 PDF 框选区域提问 | 可预览、可移除的区域截图 | 对应页，并短暂标出区域 |

PDF 页面右键菜单提供「对本页聊天」和「框选区域聊天」。框选复用同一浮窗；截图显示在输入框上方，用户可先移除截图、输入问题再发送。框选只比整页提问多保存页内坐标和截图；坐标用于重新定位，截图随 ACR 保存。原始文件不被修改。选区锚点与浮窗位置分别管理。

## ACR 文件：产品目标

ACR（Annotation Co-Reading）是 OpenRead 的开放文件格式。一份 `.acr` 计划用 ZIP 容器和 JSON 数据带走 Source 原件、所有主对话、讲解和便签、Annotation 的完整讨论与树状关系、引用和阅读附件。便签与产生它的 Annotation 关联；打开便签后应能找到对应的详细讨论。复制到另一台电脑后，OpenRead 或兼容阅读器可以在没有原目录、账号和缓存的情况下显示已有内容。凭据不进入 ACR。当前尚未实现 ACR 读写。

格式草案、读写约束和兼容性验证说明见 [`ACR/README.md`](ACR/README.md)。跨阅读器兼容性仍需用同一份公开样例验证。

## Source 初始化（已实现首版）

导入 PDF 后，可在阅读器工具栏选择「初始化」；完成后同一入口变为「查看概览」。初始化逐页读取 PDF 文本层并渲染对应页面图像，以每段不超过 12,000 个 Unicode 字符、最多 8 页图像的片段交给当前默认模型；文字和页面图像在同一轮请求中发送。没有文字层的扫描页也会以图像交给具备视觉能力的模型，不做本地 OCR。模型可在页边界建议下一段连续页码；建议无效或缺失时按程序默认方式切分。每轮结合上一版概览和新片段，更新一份正文最多 6,000 字符的 Markdown 概览；若模型输出超限，会再请求压缩一次。概览保留全文结构、各部分作用、关键概念与关系及页码定位；阅读限制和搜索来源作为独立内容呈现。ChatGPT 初始化请求使用设置中的默认思考强度；API Key 路由忽略该 ChatGPT 专属选项。概览帮助后续阅读，不等于 Explanation，也不替代原文。

ChatGPT 路由在每个原文分段更新时都可自行决定是否调用联网搜索；最后一个分段的专属提示会鼓励模型整理概览后按需补充搜索。若中途已覆盖相关内容，可以不重复搜索。API Key 路由当前不提供联网工具，会明确说明未检索外部资料。外部来源单独列出并保留链接。Sign in with ChatGPT 的搜索能力仍受模型、账号和组织策略限制。

概览暂存在当前阅读器会话中；移除 Source 或退出应用后清除，不写入 ACR，也不会自动作为聊天上下文。页面图像无法读取或模型拒绝图像输入时，会回退到可用文本并列出受影响页码；扫描页的视觉识读效果受图像质量和模型能力影响。初始化的分段、提示词、路由及接口细节见 [`desktop/backend/agent/README.md`](desktop/backend/agent/README.md)。

## 其他产品逻辑目标（待实现与细化）

以下仍是目标产品流程，不代表当前已实现：

- 主对话（Explanation）：用户可以要求概览、逐页详细讲解或自定义内容。用户也可以另开主对话，提出较大的补充知识主题；Agent 结合 Source 说明相关知识在原文中的用法。

  如果 Annotation 中的问题需要长篇补充讲解，Agent 可以建议转到主对话，由用户决定是否迁移。

- Annotation：从 Source、主对话或已有 Annotation 中选中文字/公式，也可选择 PDF 页面或框选区域，再打开同一套浮窗聊天。浮窗上下文随入口附上原文选区、页面内容或区域截图。
  - `comment` 表示想法、评论或灵感，Agent 通过交流帮助用户继续思考。
  - `question` 表示疑问，Agent 负责解释。
  - 用户不必先判断类型；Agent 在讨论中自动整理便签。问题便签按问题记录“问题 + 回答”；评论便签保留用户想法，并可补充对话中形成的有用提示。便签不替代完整 Annotation 讨论。

- ACR：采用 ZIP + JSON；目标是保存所有 Source、主对话、讲解、便签及其关系，并保留便签对应的完整 Annotation 细节，便于从便签回到原讨论。

## 技术与目录

前端使用 TypeScript、React、CSS 和 Vite；本地后端使用 Rust 和 Tauri。Source 分三处：`backend/source` 导入和解析文档并确定稳定锚点；`frontend/source` 按格式呈现原文并处理选区/框选交互；`backend/agent/source` 按格式读取、组合和切分 Agent 上下文，不实现底层解析。按格式选择成熟引擎；同一引擎能同时提供预览和文本时，优先复用已打开的文档。PDF 使用页码及页内矩形，Markdown 使用标题层级和源文本范围。

```text
desktop/
  frontend/
    app_ui/                   欢迎页、设置、双栏工作区
    source/                   按格式呈现 Source 与阅读交互
      PDF/                    PDF 阅读、文字选择和区域截图
      markdown/               Markdown/TeX 显示与公式呈现
    source_ui/                通用原文阅读 UI
    agent_ui/
      explanation_ui/         讲解入口、正文和版本切换
      annotation_ui/          选区入口、浮窗和批注摘要
      chat_ui/                共用聊天渲染、输入和模型选择
  backend/
    agent/                    ChatKit 本机模型请求和首版 Source 初始化；其他 Agent 工作流未实现
      source/                 按文件格式获取和切分 Agent 上下文
      explanation/            讲解工作流（未实现）
      annotation/              批注问答工作流（未实现）
    source/                   文档导入与解析
      PDF/                    PDF 文件分段读取和解析接口
      markdown/               Markdown 标题、章节与图片源锚点
    annotation/               批注锚点、关系和保存
    ai-verification/          ChatGPT 登录授权
    providers/                服务商 JSON、Key 验证和模型调用
    acr/                      ACR 读写
ACR/                          格式草案和兼容性样例
tests/                        应用测试和专用夹具
```

各目录的职责和约束见对应的 README。运行生成的文件和测试输出放在系统临时目录。

## 界面参考

当前视觉参考在 [`reference/approved-demo/`](reference/approved-demo/)。Demo 用于理解外观和交互；当 Demo 与本仓库 README 中的产品行为约定不一致时，以 README 为准。界面组件负责展示与交互，不把授权或模型业务流程绑定到某个视觉组件。

## 发布

正式 Windows 安装包计划通过 [GitHub Releases](https://github.com/zyc-lIIll/OpenRead/releases) 按版本分发。

本地验收安装包只用于验证当前工作区，不代表已发布版本；测试生成物放在系统临时目录，不纳入源码或 ACR。

## Windows 首版本地构建

在具备 Node.js、Rust MSVC 工具链和 Tauri Windows 构建依赖的 Windows x64 环境下，通过 PowerShell 运行：

```powershell
cd desktop/frontend
npm ci
npm run tauri -- dev
```

构建本地 NSIS 验收包时，将 Cargo 输出放到系统临时目录：

```powershell
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP "openread-target"
npm run tauri -- build
```

安装包会生成在 `$env:CARGO_TARGET_DIR\release\bundle\nsis\`。未设置 `CARGO_TARGET_DIR` 的普通构建默认写入 `desktop/backend/target/`。

在欢迎页选择 **Sign in with ChatGPT** 打开系统浏览器，按提示登录并选择是否授予额度使用权限。回到 OpenRead 后，在「设置 → 账号」查看账号状态、切换模型来源，或使用最右侧的“退出登录”和“重新登录”按钮。额度授权成功后默认选择 ChatGPT 来源；也可以关闭开关改用 API Key。欢迎页选择「使用 API Key」可直接进入阅读台。

在左侧 Source 的「＋」或「文件 → 导入 PDF」中选择一份或多份 PDF。点击文件标签切换文档，可滚动阅读、按实际页序跳转、缩放和选择文字。PDF 阅读与文本提取可离线使用。文件标签随应用退出结束，原件保留。

PDF 按页读取接口供后续的 Annotation 和 Explanation 使用。扫描页仍可阅读和截图，文字提取依赖文件本身的文本层。
