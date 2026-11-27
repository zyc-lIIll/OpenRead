# Explanation UI

`agent_ui/explanation_ui/` 是右侧 Explanation 主对话和未来讲解文档的前端职责区。当前主对话由阅读台的 [`chat_ui`](../chat_ui/README.md) 呈现，本目录尚无独立讲解正文或版本视图实现。

ChatKit 流式显示正文、临时 Activity 和可点击引用。引用跳转由 `app_ui` 激活对应 PDF Source，等待阅读器就绪后定位实际页码。模型路由、阶段转换及授权流程由后端负责。

产品目标还包括按章节阅读讲解、生成新版本并切换版本，Annotation 可关联到创建时的 Explanation 版本。上述文档、版本和关系尚未实现。后端工作流见 [`desktop/backend/agent/explanation/README.md`](../../../backend/agent/explanation/README.md)。
