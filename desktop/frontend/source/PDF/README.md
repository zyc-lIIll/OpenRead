# PDF 阅读器

`PdfViewer` 接收当前导入项的 Source ID 与文件名，在左栏显示 PDF。它把页序和缩放状态交回 `app_ui`，后者按文件标签保存。PDF.js 的 worker、字符映射、字体和解码资源随应用打包。

PDF 导入、阅读和按页解析已实现。阅读器工具栏可启动 Source 初始化，并临时查看生成的 Markdown 概览。PDF.js 负责前端渲染，也向 Rust 解析桥提供页面文字、版面信息和 PNG；后端校验请求并管理导入原件。页图不要求页面当前显示在阅读器中，PNG 最长边限制为 2,048 px，图像数据限制为 4 MiB。文字选区读取、区域截图和页面跳转接口已提供，但页/矩形选择入口及 Annotation 上下文传递尚未完成。

## 给 Annotation 和 Explanation 的接口

- `pdfReadDocument(sourceId)` 返回文档页数。`pdfReadPage(sourceId, pageNumber)` 返回单页文字、可见边界、旋转和 PDF 用户单位。页序从 1 开始，包括封面，解析不要求页面在屏幕上。
- `selectedPdfText(sourceId)` 返回当前单页文字选区、页序和页内矩形。`capturePdfRegion(sourceId, pageNumber, rect)` 返回区域 PNG，可用于文本层缺失的公式、图片和扫描页。
- `navigatePdfToPage(sourceId, pageNumber)` 跳转当前阅读器中对应的页面。调用方先激活正确的 Source 标签。
- 页内矩形以未旋转的可见页面左上角为原点。`x`、`y`、`width`、`height` 按该页面宽高归一化为 `0–1`，与滚动位置、缩放和屏幕像素密度无关。Source ID 和实际页序应与矩形一起保存。

## 选区交互规划

- **选中文字**：返回文字、页码和归一化页内矩形；当前 `selectedPdfText` 可读取浏览器文字选区。
- **选择整页**：以后支持针对当前页提问，并以实际页码定位；当前页状态由 PDF 阅读器维护。
- **矩形框选**：用户拖框选择区域，使用 `capturePdfRegion` 取得区域图像；页面序和归一化矩形作为定位锚点。该拖框交互尚未实现。所有 Source 格式暂定都要支持矩形框选。

扫描页可以阅读和截图，有文本层时才会返回提取文字。需要密码的文件会在当前标签提示。
