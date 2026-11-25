# Source 阅读 UI

`source_ui` 放置不依赖具体原文格式的阅读内容视图。目前通用原文阅读器尚未实现；`SourceOverview` 在此展示 Source 初始化生成的 Markdown 概览，原文 PDF 阅读器及其选区/截图接口位于 `desktop/frontend/source/PDF/`。页面框架和标签管理由 `app_ui` 负责。原件保持只读。

概览保存在阅读器和后端 Workspace 运行状态中，返回时 PDF 保持挂载以保留滚动位置；移除 Source 或退出应用后清除。概览不写 ACR，不会自动注入每条聊天，但 Explanation 可按需通过后端工具读取。概览正文使用 `desktop/frontend/source/markdown/MarkdownRenderer.tsx` 和 GFM、KaTeX 渲染，不解析原始 HTML；外部链接只允许 HTTP(S)，并由本机后端打开。Markdown Source 文件导入和阅读器尚未实现；标题、章节范围和图片引用锚点已由 `desktop/backend/source/markdown/` 解析。

首次没有导入 Source 时，显示一个巨大的「＋」水印。点击空白区域任意位置都可打开文件选择器并导入 Source。导入后按标签切换原文。

未来按格式使用成熟预览与解析引擎，不在 UI 中自行实现文件解析器。目标是将文字、公式、页面或框选区域交给 `agent_ui/annotation_ui`；当前尚未实现该浮窗及上下文传递。
