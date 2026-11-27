# Markdown 结构锚点

`desktop/backend/source/markdown/` 从已取得的 Markdown 文本中解析结构和资源引用，为未来 Markdown 阅读器与 Agent 提供源位置。它属于后端原文解析层，不导入文件、不渲染页面、不计算屏幕坐标，也不存储内容。

## 接口与输出

`parse(markdown)` 返回标题与图片集合：

- `headings` 保存标题等级、纯文本、标题范围和包含子标题的章节范围。章节在下一个同级或更高级标题之前结束。
- `images` 保存目标地址、标题、替代文字、Markdown 引用范围和所属标题路径。

所有源范围从 0 开始，采用左闭右开的 UTF-8 字节偏移。前端用这些偏移回到源文本；图片可见矩形由前端根据当前 DOM 和布局计算。

当前只实现结构解析。Markdown 文件导入、段落级语义锚点、前后端锚点调用和 Annotation 交互尚未接入。字段和 Source 三层职责见[后端 Source README](../README.md)及[前端 Markdown README](../../../frontend/source/markdown/README.md)。
