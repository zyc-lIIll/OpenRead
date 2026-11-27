# 应用框架 UI

`app_ui/` 负责欢迎页、设置页和双栏阅读台的页面结构、导航与页面级状态。Source 阅读呈现、聊天消息和 Agent 专属交互由对应前端模块提供。

## 当前入口与接口

- 欢迎页提供 ChatGPT 登录和直接进入阅读台使用 API Key 两种入口。
- 阅读台管理 Source 标签、PDF 当前页与缩放状态、概览查看、右侧 ChatKit 对话和引用跳转。
- 设置管理 API 服务商、ChatGPT 账号、默认模型、ChatGPT 思考强度和主题。服务商编辑表单可以提交用户输入的新 API Key；后端读取结果只返回 `hasKey`，不返回已保存的 Key。OAuth token 不交给前端。
- PDF 导入后，用户可选择初始化本次导入的 Source。初始化逐个执行，可查看进度、取消并切回概览。

当前已实现阅读台基础框架与 Explanation 主对话入口。页面没有 Explanation 文档版本管理，Annotation 浮窗尚未接入。

## 阅读台产品约定

- 左栏为 Source，右栏为 Explanation。标签切换保留每个 PDF 的页码和缩放状态。
- Source 移除只从本次工作区移除后端导入副本，原文件保留。移除非当前标签时保持当前选择；移除当前标签时优先选择右侧，再选择左侧仍存在的标签。
- 两栏可拖动或键盘调整，比例范围为 30%–70%。中央分隔线 1px 并留出可操作热区。Source 与 Explanation 标签栏高 36px，内容过多时横向滚动。
- 页面左侧导航轨宽 44px，设置按钮点击区域为居中的 36px 方形。主题仅改变应用 UI，原始文件保持自己的阅读外观。
- 这些界面尺寸和部分交互细节是产品约定。交互未实现时需在功能状态中标明。

## 设置与模型来源

设置分别保存 API 和 ChatGPT 默认模型。聊天框可切换当前对话模型；Source 初始化使用所选来源的默认模型。ChatGPT 默认思考强度支持自动、低、中、高，聊天框可覆盖当前和后续对话请求。自动省略 Responses API 参数；API Key 路由不显示或发送该选项。

账号页显示当前 ChatGPT 账号和额度授权状态，并可切换 ChatGPT/API 模型来源。关闭 ChatGPT 来源选择不会退出账号。额度授权需针对当前账号明确请求。真实登录、额度授权和模型请求必须在目标 Windows 环境验收。

## 相关模块

- [`agent_ui/chat_ui/`](../agent_ui/chat_ui/README.md) 实现阅读台共用聊天。
- [`source_ui/`](../source_ui/README.md) 呈现 Source 概览和通用原文区域。
- [`source/PDF/`](../source/PDF/README.md) 实现 PDF 阅读、页码跳转和选区接口。
- [`agent_ui/explanation_ui/`](../agent_ui/explanation_ui/README.md) 描述 Explanation 主对话入口。未来 Annotation UI 见 [`agent_ui/annotation_ui/`](../agent_ui/annotation_ui/README.md)。

视觉 Demo 在 [`reference/approved-demo/`](../../../reference/approved-demo/)，只作外观参考。产品行为以根 README 和本模块文档为准；模型路由和授权逻辑不绑定到视觉组件。
