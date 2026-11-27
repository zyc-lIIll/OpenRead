# PDF 阅读器

`source/PDF/` 显示当前 PDF，并提供页面状态、选区几何、解析桥和区域截图接口。`PdfViewer.tsx` 负责阅读交互，`PdfViewer.css` 定义样式，`document.ts` 对接 PDF.js 和解析命令，`geometry.ts` 处理归一化页内矩形。页码与缩放状态交回 `app_ui`，按 Source 标签保存。

PDF 导入、阅读、逐页文字解析与初始化图像已接入。PDF.js worker、字体、字符映射和解码资源随前端打包。文字选区和区域截图接口可用，Annotation 的页面菜单和拖框交互尚未接入。

## 阅读和解析接口

- `pdfReadDocument(sourceId)` 返回页数。
- `pdfReadPage(sourceId, pageNumber)` 返回该页文字、可见边界、旋转和 PDF 用户单位。读取不要求页面正在屏幕上。
- `selectedPdfText(sourceId)` 返回当前页文字选区、页码和归一化矩形。
- `capturePdfRegion(sourceId, pageNumber, rect)` 返回指定区域的 PNG，可用于扫描页、公式和图片。
- PDF 解析经 Rust bridge 校验 Source ID 和图像数据。Source 初始化页图最长边为 2,048 px，单张不超过 4 MiB。

页码从 1 开始且包括封面。矩形以未旋转可见页面左上角为原点，宽高归一化至 `0–1`。保存位置时将 Source ID、页码与矩形一起使用，不以缩放、滚动或屏幕像素作为稳定锚点。

## 引用跳转与 Annotation 目标

Explanation PDF 引用由 `app_ui` 激活 Source。阅读器等文档加载后验证页码并跳转。当前页状态按文件标签保留。

产品目标还包括整页提问、拖动矩形框选和向 Annotation 传递截图及锚点。选中文字已有读取接口；整页/框选入口、截图预览和 Annotation 浮窗尚未实现。扫描 PDF 有文本层时可提取文字，无文本层时可把图像交给视觉模型；当前不做本地 OCR。需要密码的文件会在当前标签报告。
