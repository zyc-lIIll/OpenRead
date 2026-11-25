# ACR 开发者公约（草案）

ACR（Annotation Co-Reading）用于把原始材料和阅读成果装进一份可传送、可由兼容阅读器读取的文件。当前方案是 ZIP 容器和 JSON 元数据；本文件描述草案方向，不代替尚未完成的正式格式规范。应用中的 ACR 导入/导出尚未实现。

## 容器示例

```text
物理笔记.acr
├── manifest.json        格式版本、材料清单和相对路径
├── sources/             原始 PDF、PPTX 等文件
├── conversations/       主对话及 Annotation 详细讨论
├── explanations/        Explanation 文档/版本
├── annotations.json     批注、便签、锚点和父子关系
└── attachments/         框选截图等阅读附件
```

## 数据关系

- `manifest.json` 声明格式版本、稳定 ID 和容器内文件路径。
- Source 保留原文件；所有主对话、Explanation 和便签均持久化，并可引用原文位置。
- Annotation 记录类型（`comment` 或 `question`）、锚点、完整讨论及父子关系。Agent 生成的便签与产生它的 Annotation 关联；用户应能从便签找到详细讨论。子批注可指向父讨论中的具体回复。
- 定位信息按格式选择合适方式，例如 PDF 原文 ID 与实际页序，或章节、段落锚点。PDF 框选额外保存页内坐标和截图。
- 引用锚点由 Source 的稳定 ID 和格式锚点组成。PDF 首版锚点为 `{ "source_id": "<manifest source id>", "kind": "pdf_page", "page": 12 }`；未来需要定位页内选区时，再附加基于未旋转页面左上角归一化的矩形和可选原文范围。临时 Agent 回复中的 `rN` 仅在单次回复内有效，不能作为 ACR 引用 ID 持久化。
- 一条 Explanation 引用还应保存其对话/讲解片段的引用位置（例如片段内字符范围）以及上述原文锚点。呈现时可把引用位置渲染为可点击实体；源文件和锚点决定跳转，聊天文本偏移不决定原文定位。
- ACR 不保存账号凭据或 API Key。阅读所需的图片等附件随包保存。

上述文件布局和数据关系仍是产品目标，具体 JSON schema 与正式兼容性规范尚待确定。

当前运行时已使用 PDF 页码锚点，并通过 ChatKit EntitySource 的 `data.sourceId` 与 `data.pageNumber` 实现会话内跳转；该映射尚未持久化到 ACR。将来 ACR 的 Source ID 应由 `manifest.json` 稳定分配并在导入、导出间保持一致。回复流中的 `[[cite:rN]]` 由后端校验后转成实体注释，临时编号及其在回答中的字符位置不能替代 ACR 持久锚点。

## 读写要求

阅读器应校验容器、格式版本、路径和引用；拒绝越界路径并限制解包大小。写入失败时保留已有文件。容器内路径相对于 ACR 根目录。

实现位于 [`desktop/backend/acr/`](../desktop/backend/acr/README.md)。格式兼容性尚未经过跨阅读器实测；正式规范和公开样例完成后，应使用同一份 ACR 验证导出、干净环境重开和独立实现。
