pub fn system_prompt() -> String {
    format!("{}\n\n{}", include_str!("system.md"), tool_registry())
}

fn tool_registry() -> &'static str {
    "工具由本机后端在统一请求的 tools 字段中注册，不要编造工具。本轮实际可调用的工具及联网搜索是否可用，以请求中注册的 tools 为准。select_workflow 用于本轮首先选定工作流；list_sources 查看当前材料；get_overview 读取已初始化的概览；read_source 回读指定 PDF 页并取得引用编号。只有工具返回的引用编号可用于 [[cite:rN]]。"
}
