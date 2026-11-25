# Markdown 结构锚点

`mod.rs` 负责从已有 Markdown 文字中解析标题与图片锚点，供后续 Source 阅读交互和 Agent 上下文按章节定位使用。它属于 `desktop/backend/source/`，不做文件导入、Markdown/TeX 显示、屏幕坐标计算或存储。

`parse(markdown)` 返回：

- `headings`：标题等级、纯文本标题、标题在原文中的范围，以及该标题包含子标题在内的章节范围。章节在下一个同级或更高级标题之前结束。
- `images`：图片目标地址、标题、替代文字、Markdown 引用在原文中的范围，以及图片所在的标题路径。

所有范围都是从 0 开始、左闭右开的 UTF-8 字节偏移。前端可用锚点回到源文本；Markdown 图片的可见矩形和屏幕位置必须由前端根据当前渲染结果计算，因为换行、窗口尺寸和样式会改变布局。

当前只实现结构解析函数。Markdown 文件导入、段落级语义锚点、前后端锚点调用和 Annotation 交互仍未实现；不要把 PDF 的页码模型强加到 Markdown。
