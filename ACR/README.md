# ACR 开发者公约（草案）

ACR（Annotation Co-Reading）用于把原始材料和阅读成果装进一份可传送、可由兼容阅读器读取的文件。当前方案是 ZIP 容器和 JSON 元数据；本文件描述草案方向，不代替尚未完成的正式格式规范。

## 容器示例

```text
物理笔记.acr
├── manifest.json        格式版本、材料清单和相对路径
├── sources/             原始 PDF、PPTX 等文件
├── explanations/        各版本讲解数据
├── annotations.json     批注、对话和父子关系
└── attachments/         框选截图等阅读附件
```

## 数据关系

- `manifest.json` 声明格式版本、稳定 ID 和容器内文件路径。
- Source 保留原文件；Explanation 作为独立版本保存，并可引用原文位置。
- Annotation 记录类型（`comment` 或 `question`）、锚点、完整讨论及父子关系。子批注可指向父讨论中的具体回复。
- 定位信息按格式选择合适方式，例如 PDF 原文 ID 与实际页序，或章节、段落锚点。PDF 框选额外保存页内坐标和截图。
- ACR 不保存账号凭据或 API Key。阅读所需的图片等附件随包保存。

## 读写要求

阅读器应校验容器、格式版本、路径和引用；拒绝越界路径并限制解包大小。写入失败时保留已有文件。容器内路径相对于 ACR 根目录。

实现位于 [`desktop/backend/acr/`](../desktop/backend/acr/README.md)。格式兼容性尚未经过跨阅读器实测；正式规范和公开样例完成后，应使用同一份 ACR 验证导出、干净环境重开和独立实现。
