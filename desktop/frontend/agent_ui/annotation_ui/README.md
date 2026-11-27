# Annotation UI 规划

`agent_ui/annotation_ui/` 规划选区入口、Annotation 浮窗聊天、便签显示和讨论恢复。目前没有对应 UI 实现，也未将 PDF 选区接口和基础 ChatKit 连接成 Annotation 工作流。

## 目标交互

- 从原文或讲解中的完整文字/公式、PDF/PPT 页面或 PDF 框选区域开启讨论。框选将截图作为可移除的输入附件。
- 锚点描述原文位置；浮窗屏幕坐标单独管理。用户关闭浮窗后可恢复讨论，也可从具体回复继续追问。
- `comment` 与 `question` 表示便签类型。用户无需预先分类，Agent 依据讨论整理便签；便签关联到完整讨论和锚点。
- 浮窗复用 [`../chat_ui/`](../chat_ui/README.md) 的消息呈现，Annotation 自己传入入口上下文和关系。

## 当前实现

PDF 的文字选区读取与区域截图接口已存在，页/区域入口、浮窗、附件预览、便签和上下文传递未实现。后端 Annotation Agent 规划见 [`desktop/backend/agent/annotation/README.md`](../../../backend/agent/annotation/README.md)。
