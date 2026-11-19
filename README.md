# OpenRead

OpenRead 是一款以文档为单位的 AI 辅助阅读器，最终目标是 Windows 桌面应用。用户把教材、课件或笔记放进同一个工作区，左侧阅读原文，右侧阅读讲解；遇到疑问时，从原文或讲解打开浮窗继续讨论。

## 阅读工作区

- **Source** 保存并只读呈现原始文件。可以导入多份材料并用标签切换。目标格式包括 PDF、Markdown、TXT、EPUB、DOC/DOCX 和 PPT/PPTX。
- **Explanation** 是独立的讲解文档，可生成概览、详细讲解或自定义内容。每次生成都建立新版本；讲解可按章节阅读，并通过引用回到原文。
- **Annotation** 是关联到原文、某版讲解或已有回复的评论与问题。关闭浮窗后可从摘要恢复讨论，也可以在回复上继续追问。

原文保持只读。重新生成讲解不会覆盖旧版本，已有批注仍指向创建时关联的原文或讲解版本。

## 聊天入口与模型路由

Explanation 对话和 Annotation 浮窗使用同一套聊天交互。用户选择的 ChatGPT 账户或 API 服务商决定模型请求走哪条渠道，不改变聊天界面。

- 最多登录一个 ChatGPT 账号。登录后默认使用该账号的 ChatGPT 额度；用户仍可在设置中切换为优先使用 API 服务商。
- 未登录时，使用用户配置的服务商和模型。欢迎页选择「使用 API Key」可先进入阅读台，之后再配置服务商。
- 模型选择器显示「服务商名称 + 模型」；选择结果决定实际请求路由。ChatGPT 模型还可选择思考强度。
- 服务商配置保存在本机 JSON 中，包含名称、Base URL、API Key 和模型。界面用星号遮住 API Key；遮罩只影响显示，不加密 JSON 文件。密钥不得写入 ACR、日志、样例或 Git。

## 批注与 PDF 选区

产品只使用一种浮窗聊天，随入口附上不同上下文：

| 入口 | 带入聊天的内容 | 批注打开时的定位 |
| --- | --- | --- |
| 选中文字或整条公式 | 引用文字 | 原文中的对应选区 |
| 对 PDF 或 PPT 当前页提问 | 当前页内容 | 对应原文页 |
| 对 PDF 框选区域提问 | 可预览、可移除的区域截图 | 对应页，并短暂标出区域 |

PDF 页面右键菜单提供「对本页聊天」和「框选区域聊天」。框选复用同一浮窗；截图显示在输入框上方，用户可先移除截图、输入问题再发送。框选只比整页提问多保存页内坐标和截图；坐标用于重新定位，截图随 ACR 保存。原始文件不被修改。选区锚点与浮窗位置分别管理。

## ACR 文件

ACR（Annotation Co-Reading）是 OpenRead 的开放文件格式。一份 `.acr` 计划用 ZIP 容器和 JSON 数据带走原件、讲解版本、批注树、引用和阅读附件。复制到另一台电脑后，OpenRead 或兼容阅读器可以在没有原目录、账号和缓存的情况下显示已有内容。凭据不进入 ACR。

格式草案、读写约束和兼容性验证说明见 [`ACR/README.md`](ACR/README.md)。跨阅读器兼容性仍需用同一份公开样例验证。

## 技术与目录

前端使用 TypeScript、React、CSS 和 Vite；本地后端使用 Rust 和 Tauri。后端导入原件并管理文档数据，前端负责呈现和阅读交互。按格式选择成熟的预览与解析引擎；同一引擎能同时提供预览和文本时，优先复用已打开的文档。共用接口描述格式能力，具体定位使用页码、章节或段落等合适的锚点。

```text
desktop/
  frontend/
    app_ui/                   欢迎页、设置、双栏工作区
    source_ui/                原文预览与阅读交互
    agent_ui/
      explanation_ui/         讲解入口、正文和版本切换
      annotation_ui/          选区入口、浮窗和批注摘要
      chat_ui/                共用聊天渲染、输入和模型选择
  backend/
    agent/                    共享 Agent 循环
      explanation/            讲解工作流
      annotation/              批注问答工作流
    source/                   文档导入与解析
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
