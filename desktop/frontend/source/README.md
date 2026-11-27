# Source 格式阅读 UI

`desktop/frontend/source/` 按原文格式放置阅读器、渲染和阅读选区交互。该层负责当前页面的呈现与几何信息。导入、解析和持久定位归后端 `desktop/backend/source/`；模型上下文读取归后端 `desktop/backend/agent/tool/source/`。页面框架、标签和通用概览由 `app_ui/` 与 `source_ui/` 管理。

## 子模块和状态

- [`PDF/`](PDF/README.md) 实现已支持的 PDF 阅读、页码与缩放状态、文字选区及区域图像接口。整页/框选入口尚未实现。
- [`markdown/`](markdown/README.md) 使用 `MarkdownRenderer.tsx` 呈现 Source 初始化概览的 Markdown / TeX。Markdown Source 文件导入和独立阅读器尚未实现。

未来各格式为 Annotation 提供文字、公式、页面或矩形区域上下文，以及各自稳定锚点。所有格式计划支持区域选择，但持久定位使用格式原生结构，不能统一强加 PDF 页码模型。屏幕位置与稳定锚点分别处理。
