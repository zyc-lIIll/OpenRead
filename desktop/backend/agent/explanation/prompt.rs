pub fn system_prompt(can_search: bool) -> String {
    format!(
        "{}\n\n{}",
        include_str!("system.md"),
        tool_registry(can_search)
    )
}

fn tool_registry(can_search: bool) -> &'static str {
    if can_search {
        "工具由本机后端在 API 请求的 tools 字段中注册，不要编造工具。select_workflow 用于本轮首先选定工作流；list_sources 查看当前材料；get_overview 读取已初始化的概览；read_source 回读指定 PDF 页并取得引用编号；本轮可按需联网搜索。只有工具返回的引用编号可用于 [[cite:rN]]。"
    } else {
        "工具由本机后端在 API 请求的 tools 字段中注册，不要编造工具。select_workflow 用于本轮首先选定工作流；list_sources 查看当前材料；get_overview 读取已初始化的概览；read_source 回读指定 PDF 页并取得引用编号。本轮没有联网搜索工具，不得声称已检索外部资料。只有工具返回的引用编号可用于 [[cite:rN]]。"
    }
}
