# Source 内容视图

`source_ui/` 放置与具体原文格式无关的内容视图。当前实现 `SourceOverview.tsx` 和对应样式，用于呈现 PDF Source 初始化生成的 Markdown 概览。通用原文阅读器尚未实现，PDF 原文由 [`source/PDF/`](../source/PDF/README.md) 呈现。

概览由 `app_ui` 管理其打开状态。Markdown 正文复用 `source/markdown/MarkdownRenderer.tsx`，使用 GFM 和 KaTeX 呈现，不执行原始 HTML。概览外部链接经过 HTTP(S) 校验后由本机后端打开。联网来源以独立链接列表展示。

概览只保存在阅读器和后端 `Workspace` 运行状态中。退出概览时 PDF 阅读器保持挂载以保留阅读位置；移除 Source 或退出应用后清除概览。它不写入 ACR、不自动进入每条聊天，Explanation 可按需调用 `get_overview` 读取。

本目录当前只有通用概览视图。页面框架、Source 标签和切换状态由 [`app_ui/`](../app_ui/README.md) 管理；格式阅读器按子目录放在 [`source/`](../source/README.md)。Annotation 选区与浮窗上下文尚未接入。
