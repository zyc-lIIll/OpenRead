# Explanation Agent（未实现）

这是目标模块，当前没有 Explanation Agent 工作流。右侧聊天可以进行基础模型对话，但不会读取 Source、组装文档上下文、生成 Explanation 文档/版本或生成原文引用。

目标行为：根据 Source 和用户要求生成概览、详细讲解或自定义讲解，并提供可回到原文的引用。每次生成建立新的 Explanation 版本，不覆盖旧版本；已有 Annotation 继续关联原版本。实现时复用 `../` 的模型请求能力，并在本目录加入真正的提示词、上下文和工作流逻辑。
