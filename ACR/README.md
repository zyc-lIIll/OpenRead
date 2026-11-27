# ACR 开发者公约草案

ACR（Annotation Co-Reading）是用于交换原始材料和阅读成果的开放档案格式。产品目标是把 Source、Explanation、Annotation 及其稳定关系放入一份可移交的 `.acr` 文件，让兼容阅读器在没有原文件目录、账号或运行缓存时仍能显示已保存内容。

当前方案使用 ZIP 容器和 JSON 元数据。本文件记录格式方向和安全边界，不是已冻结的 schema。OpenRead 目前尚未实现 ACR 导入或导出。

## 目标容器结构

```text
reading.acr
├── manifest.json        格式版本、稳定 ID 和容器内相对路径
├── sources/             导入时保留的原始文件
├── conversations/       Explanation 和 Annotation 完整对话
├── explanations/        Explanation 文档及版本
├── annotations.json     便签、锚点和父子关系
└── attachments/         区域截图等阅读附件
```

上述目录名和 JSON 结构仍是讨论中的草案，正式实现前需要确定兼容规范。

## 数据和关系

- `manifest.json` 声明 ACR 格式版本、稳定来源 ID 和容器内相对路径。相同 Source 在导入、导出间保持同一稳定 ID。
- 档案保存原始 Source、主对话、Explanation 文档及版本、Annotation 便签、完整讨论和附件。便签能回到生成它的讨论；子批注可引用父讨论中的具体回复。
- Annotation 的类型为 `comment` 或 `question`。用户不必预先指定，Agent 可根据讨论整理便签。
- 锚点由稳定 Source ID 和格式原生位置组成。PDF 使用页码，未来选区可附带归一化矩形或原文范围；Markdown 可使用标题路径及 UTF-8 源范围。临时屏幕位置不作为持久锚点。
- PDF 页码从 1 开始且包含封面。页内矩形以未旋转页面左上角为原点，按可见页面宽高归一化到 `0–1`。
- Explanation 的正文引用还需要保存其在对应文档或片段内的位置，以及跳转所用的 Source 锚点。单次 Agent 回复中的 `[[cite:rN]]`、`[[cite:wN]]` 编号只是运行期临时值，不能写成 ACR ID。
- 账号凭据和 API Key 不进入 ACR。阅读所需的图片、截图等附件随档案保存。

当前 ChatKit EntitySource 使用 Source ID 与 PDF 页码，实现会话内跳转。运行期 UUID 和临时引用尚未持久化；将来由 manifest ID 映射到稳定格式锚点。

## 读写和兼容边界

读取方校验 ZIP 容器、格式版本、路径和引用关系，拒绝越界路径并限制解包总量。容器内路径始终相对于 ACR 根目录。写入期间出现错误时保留现有文件。

正式规范和公开样例确定后，使用同一份样例分别验证导出、干净环境重开和独立实现兼容性。跨阅读器兼容目前尚未验收。

## 实现位置

`ACR/` 保存格式公约和兼容样例。未来应用读写位于 [`desktop/backend/acr/`](../desktop/backend/acr/README.md)，与账号凭据和服务商配置分开。当前该后端目录尚无读写实现。
