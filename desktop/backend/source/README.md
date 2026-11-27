# Source 导入与解析

`desktop/backend/source/` 管理本次运行期间导入的原件、格式解析接口和可回到原文的稳定锚点。它不负责 UI 呈现，也不负责把内容整理成模型上下文。

## Source 的三段职责

| 区域 | 职责 |
| --- | --- |
| `desktop/backend/source/` | 导入原件、校验读取请求、提供格式解析和稳定锚点 |
| `desktop/frontend/source/` | 呈现原文，执行选区和屏幕区域交互 |
| `desktop/backend/agent/tool/source/` | 以格式专属范围读取和切分 Agent 上下文 |

后端通过 `Workspace` 分配运行期 Source UUID 并维护只读原件副本。前端只使用 Source ID 调用后端接口，不直接传任意路径。Source 移除只清理工作区副本，用户原文件保留。

## 格式模块

- `PDF/` 暴露分页读取和异步解析桥接。PDF.js 在前端提供文字、版面信息和 PNG 页面图像；Rust 校验回传数据并管理原件。扫描页没有文字层时仍可将页面图像交给视觉模型，当前不做本地 OCR。
- `markdown/` 使用 `pulldown-cmark` 从 Markdown 文本解析标题层级、标题和章节范围、图片引用及所属标题路径。范围是从 0 开始、左闭右开的 UTF-8 字节偏移。该模块不导入 Markdown 文件、不渲染页面。

当前只有 PDF 文件可导入。Markdown 锚点解析已实现，但 Markdown 文件导入/阅读、TXT、EPUB、DOC/DOCX、PPT/PPTX 导入尚未实现。新增格式时优先选用成熟解析引擎，锚点按格式选择页码、章节或段落结构。

解析字段和范围约定见 [Markdown 锚点 README](markdown/README.md)。Agent 按页读取 PDF 的细节见 [Agent Source 上下文 README](../agent/tool/source/README.md)。
