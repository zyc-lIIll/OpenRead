# Source 导入与解析

`desktop/backend/source/` 负责文档导入、底层解析接口和原文定位锚点。它不负责前端呈现，也不负责给 Agent 组织上下文。OpenRead 的 Source 分成三处：

- `desktop/backend/source/`：导入文档、解析各格式内容，并确定可回到原文的位置锚点，例如 PDF 页序和 Markdown 标题层级。
- `desktop/frontend/source/`：呈现解析结果，并负责阅读时的选中文字、页面或区域等交互。
- `desktop/backend/agent/source/`：按格式从解析结果获取、组织和切分模型上下文，不实现底层文件解析。

当前仅支持 PDF 导入。顶层维护本次运行期间的只读原文件副本；`PDF/` 暴露分页读取和异步解析桥接。PDF.js 在前端运行，提供 PDF 文字、版面信息及 PNG 页面图像，后端校验请求并提供 Source 读取接口。扫描页没有文字层时返回空文字，可将页面图像交给视觉模型；当前不做本地 OCR。

`markdown/` 使用 `pulldown-cmark` 从 Markdown 文字中提取分级标题、标题原文范围、章节范围以及图片引用范围和所属标题路径。它定义结构锚点，不导入 Markdown 文件、不渲染 HTML，也不提供持久化。Markdown 导入尚未实现。范围为源文本中从 0 开始、左闭右开的 UTF-8 字节偏移。

Markdown 结构解析已实现，但 Markdown Source 文件导入尚未实现；TXT、EPUB、DOC/DOCX、PPT/PPTX 的导入和解析也未实现。这些是产品目标格式，当前只为 PDF 与 Markdown/TeX 编写了格式专属架构说明。按格式选成熟引擎，不自行重写通用文档解析器。共用接口可以描述格式能力，但定位结构按格式选择页码、章节、段落或合适的稳定锚点。

Markdown 锚点字段见 [`markdown/README.md`](markdown/README.md)。
