# Explanation UI

阅读台右侧目前通过 [`../chat_ui/`](../chat_ui/README.md) 提供 Explanation 主对话入口。ChatKit 展示流式回复、可点击的原文页引用和模型选择；引用点击由 `app_ui` 激活对应 Source，并等待 PDF 阅读器就绪后跳页。

独立 Explanation 文档、按章节阅读、版本生成/切换和与 Annotation 的版本关系尚未实现。当前完整对话只保留在运行期 ChatKit 线程中。
