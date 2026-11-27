# Markdown / TeX 阅读呈现

`source/markdown/` 负责 Markdown 内容的前端呈现和未来的选区交互，不负责文件导入或章节锚点解析。当前 `MarkdownRenderer.tsx` 被 Source 概览复用，使用 `react-markdown`、GFM、`remark-math` 与 KaTeX 插件。独立 Markdown Source 阅读器尚未实现。

## 渲染规则

- 按 Markdown 结构呈现标题、段落、表格、链接、图片和代码。原始 HTML 不执行。
- 外部链接仅开放 HTTP(S)，并由 Rust 后端打开。页面内锚点可保留。
- 行内公式使用 `$...$` 或 `\(...\)`，块级公式使用 `$$...$$` 或 `\[...\]`。单行块级公式需要独占一行并保留引用或列表前缀；多行公式交由 Markdown 解析器识别。
- 代码节点始终显示为代码，即使围栏语言写作 `math`。公式仅改变显示，源 Markdown/TeX 不修改。

## 锚点和选区

后端 [`source/markdown/`](../../../backend/source/markdown/README.md) 提供标题层级、章节范围、图片源文本范围和标题路径。Markdown 导入接入后，前端可据此实现标题导航与回到源文本。

未来 Annotation 选区需要返回所选文字及 UTF-8 源范围、完整公式及其标题路径，或者当前渲染区域对应的截图和 Markdown 锚点。公式选择应覆盖完整 TeX 表达式。图片屏幕矩形按当前 DOM 计算，不能作为稳定源锚点保存。各格式通用的职责边界见[前端 Source 阅读 UI](../README.md)。

当前只有 Source 初始化概览复用 Markdown 渲染器。Markdown 文件导入、正文阅读器、公式选区和 Annotation 上下文传递尚未实现。
