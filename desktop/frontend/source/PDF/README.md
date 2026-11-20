# PDF 阅读器

`PdfViewer` 接收当前导入项的 Source ID 与文件名，在左栏显示 PDF。它把页序和缩放状态交回 `app_ui`，后者按文件标签保存。PDF.js 的 worker、字符映射、字体和解码资源随应用打包。

PDF 导入、阅读和解析已实现。下列选区、截图及跳转接口可供阅读交互使用，但尚未连接到 Agent 或 Annotation 对话上下文。

## 给 Annotation 和 Explanation 的接口

- `pdfReadDocument(sourceId)` 返回文档页数。`pdfReadPage(sourceId, pageNumber)` 返回单页文字、可见边界、旋转和 PDF 用户单位。页序从 1 开始，包括封面，解析不要求页面在屏幕上。
- `selectedPdfText(sourceId)` 返回当前单页文字选区、页序和页内矩形。`capturePdfRegion(sourceId, pageNumber, rect)` 返回区域 PNG，可用于文本层缺失的公式、图片和扫描页。
- `navigatePdfToPage(sourceId, pageNumber)` 跳转当前阅读器中对应的页面。调用方先激活正确的 Source 标签。
- 页内矩形以未旋转的可见页面左上角为原点。`x`、`y`、`width`、`height` 按该页面宽高归一化为 `0–1`，与滚动位置、缩放和屏幕像素密度无关。Source ID 和实际页序应与矩形一起保存。

扫描页可以阅读和截图，有文本层时才会返回提取文字。需要密码的文件会在当前标签提示。
