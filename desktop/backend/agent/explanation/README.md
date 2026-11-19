# Explanation Agent

该目录负责 Explanation 的提示词、上下文管理和工作流。输入关联的 Source 内容及用户要求，生成概览、详细讲解或自定义讲解，并携带可回到原文的引用。

每次生成建立新的 Explanation 版本，不覆盖旧版本；已有批注继续关联原来的版本。初始阶段复用 `../` 的最小流式 Agent 循环。
