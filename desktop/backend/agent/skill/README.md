# 跨学科阅读视角

`agent/skill/` 根目录保存可供多个 Agent 使用的共用 skill。这里的文档描述阅读角度，不负责选择对话任务或控制 Agent loop。Agent 专属 skill 放在对应子目录，例如 `explanation/`。

当前包含数学/理科、人文社科、理论论文、实验与工程论文四类 Markdown 文件。通用视角的默认文字在 Explanation 选择逻辑中内置。所有文件在编译时包含，当前没有用户编辑或目录覆盖机制。

Explanation 的任务类别提示词位于 [`explanation/`](explanation/README.md)，由 `agent/tool/explanation/` 的 `select_workflow` handler 选择并加载。
