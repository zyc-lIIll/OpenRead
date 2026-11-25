# Explanation 工作流 skills

每条主对话先由 `select_workflow` 选择一个工作流：Source 讲解、知识问题、伴读讨论、复盘或其他。后端按模型返回的枚举加载对应 Markdown，并与选中的学科阅读视角一起返回给模型，避免把五套任务提示词同时注入每一轮。

全局安全规则、工具用法、引用格式和工作流选择入口写在 [`../system.md`](../system.md)。学科阅读 skills 在 [`../../skill/`](../../skill/README.md)。当前 prompt 是静态内置文件，可由开发者修改；尚未实现用户配置存储、覆盖顺序或编辑 UI。
