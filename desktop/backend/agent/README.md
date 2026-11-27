# Agent 共用架构与显示公约

`agent/` 连接本机 ChatKit 服务、模型请求、Source 初始化和对话 Agent。共用代码集中处理模型协议边界、工具注册、对话显示事件与流式引用。每个交互 Agent 管理自己的 loop、system instructions、工具集、上下文和结束条件。

## 共享组件

| 组件 | 职责 | 当前使用者 |
| --- | --- | --- |
| `model/` | 将 OpenRead 统一请求转换为服务商请求，解码流并返回统一事件和结果 | Source 初始化、Explanation |
| `tool/` | 根目录提供共用注册与 Source 工具；各 Agent 子目录提供专属工具 | Explanation；未来 Annotation 可组合需要的工具 |
| `skill/` | 根目录保存共用阅读视角，Agent 子目录保存专属 Markdown 指引 | Explanation；未来 Annotation 可添加自己的指引 |
| `CompletionEvent` | 将临时 Activity、正式正文、引用、片段完成、用户操作、完成和错误传给 ChatKit | 当前 Explanation |
| `citation.rs` | 缓冲跨 chunk 引用标记，检查引用目标，计算正文位置 | 当前 Explanation |
| `chatkit.rs` | 本机 ChatKit 协议、运行期线程、事件转发、消息组装和引用注释 | 当前阅读台主对话 |

Source 初始化是独立流水线，有自己的进度、取消和结果接口。它共用 `model/` 和 PDF 上下文读取逻辑，但不使用对话 Agent 的显示事件。Annotation 目标将复用模型边界、对话显示事件和流式引用能力，并实现自己的 loop、请求策略及结束规则。

## 文件与子目录

```text
agent/
  mod.rs                 模型路由目录、Agent 状态、CompletionEvent、对话入口
  chatkit.rs             本机 ChatKit HTTP/SSE 服务和运行期线程
  citation.rs            对话正文中的 rN / wN 标记解析与位置计算
  model/                 统一请求/响应和各服务商适配器
  tool/                  共用工具注册、调度及按格式获取 Source 上下文
    markdown.rs          编译期白名单加载内置 Markdown 指引
    explanation/         Explanation 专属工作流选择和阶段控制工具
    source/              共用的按格式 Agent 上下文读取器
  skill/                 多 Agent 共用的阅读视角
    explanation/         Explanation 专属工作流 Markdown 指引
  initial/               PDF Source 初始化和概览更新流水线
  explanation/           Explanation 入口、基础 system prompt 和独立 loop
  annotation/            Annotation Agent 目标，目前只有说明文档
```

`tool/` 和 `skill/` 根目录只放多个 Agent 共用的实现或内容；专属项放在对应子目录。Markdown 指引由 `tool/markdown.rs` 的编译期白名单加载，不接受模型或调用方提供的文件路径。Annotation 未来应在 `annotation/` 独立实现 loop，并在 `tool/annotation/`、`skill/annotation/` 放自己的工具和长指引。

各目录内 Markdown 文件的业务行为见相邻 README。运行时提示词和 skill 文件是模型指令内容，不作为架构文档维护。

## 共用显示事件

双阶段显示流程和抽象图统一记录在[根 README 的 Agent 双阶段显示公约](../../../README.md#agent-双阶段显示公约)。每条对话 Agent 回复从阶段 1 开始；阶段信号、首轮工具、循环和上下文由各 Agent 实现。

### 事件和数据边界

| 事件或数据 | 用途 |
| --- | --- |
| `Activity` | 暂时呈现阶段 1 的公开过程文字、搜索进度和实际工具状态，不写入助手正文 |
| `Delta` | 把阶段 2 的正式正文追加到当前助手消息 |
| `Citation`、`SectionDone` | 完成一个正文片段并附上引用 |
| `OfferUserAction`、`Complete`、`Error` | 请求 UI 提供用户操作，或结束/报告失败 |
| 工具参数、工具结果、模型 `Context` | 留在后端请求循环，不作为临时播报显示 |

正式输出开始前，Agent 清理当前片段的 Activity。Agent 可以为多个片段反复切换阶段。取消或错误时清理临时播报，保留已完成正文和引用。供应商的隐藏推理及不透明续传数据不作为 Activity。

`CompletionEvent` 定义在 `agent/mod.rs`，ChatKit 在 `chatkit.rs` 转为前端事件；Chat UI 负责显示。Agent 决定事件何时发出，UI 不推断业务阶段。

## 统一模型请求与响应

模型公约详见[模型 README](model/README.md)。工作流只构造 OpenRead 请求并消费统一响应。`model/mod.rs` 固定任务开始时的路由快照，并在每次请求时重新检查当下凭据或服务商配置。`chatgpt.rs` 适配 ChatGPT Responses，`other.rs` 适配通用 Chat Completions。服务商专属的请求 JSON、流帧和续传项目留在适配器及模型层。

模型返回完整本机函数调用后，由 Agent 按本轮开放的注册表子集校验，再交给 `tool::dispatch`。函数结果以统一 `function_call_output` 回填 `input`。`web_search` 是搜索意图。ChatGPT 适配器可映射到内建搜索；通用 API 当前没有搜索实现，必须明确告知模型。

## 调用位置

- Source 初始化从 `initial/mod.rs` 构造统一请求，使用 `model/`，通过独立接口报告进度和概览结果。
- ChatKit 主对话在 `chatkit.rs` 调用 Explanation 入口。`explanation/agent_loop.rs` 管理 Explanation 的工具轮次、阶段和上下文；共用工具来自 `tool/`，正文引用交由 `citation.rs` 解析。
- 前端 `agent_ui/chat_ui/` 接收 ChatKit 输出并呈现 Activity、正文、引用和用户确认操作。当前 Annotation 尚未接入这些组件。

Explanation 专属阶段流程和实现示例见 [Explanation README](explanation/README.md)。Source 初始化的分段、图像回退与取消见 [initial README](initial/README.md)。

## 其他结构简化候选

以下候选仍需结合调用点和回归范围评估。提示词拼接已并入 Explanation 入口，工作流及学科指引选择已并入 Explanation 专属工具模块。

| 优先级 | 观察 | 建议及回归重点 |
| --- | --- | --- |
| 高 | `agent/mod.rs::stream_completion` 只转发参数、调用 `explanation::stream` 并转换错误。当前唯一调用者是 `chatkit.rs`。 | ChatKit 直接调用 Agent 入口；确实需要多个 Agent 分派时再加入 dispatch。验收模型路由错误、Agent 错误、正常完成和取消事件。 |
| 中 | `tool/overview.rs::get` 只有一个调用点，负责从 `Workspace` 取 Source 名称和概览并包装 JSON。 | 把读取与结果组装并入 `get_overview` handler。验收无效 Source ID、已移除 Source、已初始化和未初始化返回。 |
| 中 | `read_source` handler 已把页码反序列化为 `u32`，随后重新打包 JSON 交给 `page_range` 再次解析。 | 改为接受类型化页码参数并在一个位置校验正值、顺序及五页上限。验收 UUID、页码边界和工具输出上限。 |
| 低 | 两个模型适配器的 `error_rejects_images` 只是转调同一个父模块函数，通用 API 的 `request_error` 也只转发 HTTP 状态。 | 直接调用模型层共用函数。ChatGPT 的思考强度专属报错仍放在 ChatGPT 适配器。验收图片拒绝回退、非图片错误、HTTP 状态和显式思考强度提示。 |
| 低 | `chatkit.rs` 在模型错误和流意外结束分支重复清理 Activity、完成当前正文并保存已有内容。 | 把相同的部分回复收尾放在一个 ChatKit 私有 helper 中，同时保留不同错误事件。验收取消、模型错误、断流、已完成引用和部分正文恢复。 |

`explanation/agent_loop.rs` 承担完整业务状态机，包括阶段工具选择、截断续写、引用、取消和工具循环。它有独立的状态和控制职责，保留为 Explanation 专属实现文件较清晰。`explanation/mod.rs` 负责固定路由、在本模块明确拼接 `system.md` 与短工具可用性规则、注册工作流工具、启动 loop，以及将完成/取消转换为共用事件。

`agent/citation.rs` 负责流式标记解析及正文位置计算。`tool/` 里没有第二个引用解析器；`tool/mod.rs::SourceCitation` 只是 `read_source` 返回的 PDF Source ID、Source 名称和页码数据。前者处理“引用标记在正文哪里”，后者描述“引用跳到哪一页”，暂时不应把二者当作重复实现。等 Annotation 的引用目标结构明确后，再决定是否让它直接返回共用目标类型。

`tool/source/mod.rs` 是 Rust 子模块声明入口，负责暴露 PDF 上下文 reader。把这类模块入口移到父文件只会改变代码位置，不会减少跨模块的数据转换。`agent/model/mod.rs` 也是共享协议路由、SSE 帧读取和取消处理的实际实现主体，两个适配器的协议职责仍应分开。

`tool/source/pdf.rs` 同时为 Source 初始化和 `read_source` 提供页读取、页码建议和分段函数，是按格式组织的实际共用逻辑。`model/` 下两个协议适配器承担不同的请求和流解析，保留独立文件。`initial/mod.rs` 虽较长，但其分段、压缩、重试、取消和进度属于一条流水线，目前没有只做传递的额外子层。

## 当前状态

已实现 ChatKit 本机服务、模型统一请求与两种适配器、Source 初始化、共用工具注册、Explanation loop、共用显示事件和流式引用解析。只有 Explanation 当前接入对话显示公约。Annotation loop、会话持久化、Explanation 版本文档和 ACR 读写尚未实现。Windows 安装包、真实账号授权和模型请求按根 README 中的验收状态记录。
