pub fn system_prompt() -> String {
    format!("{}\n\n{}", include_str!("system.md"), tool_registry())
}

fn tool_registry() -> &'static str {
    "工具由本机后端在统一请求的 tools 字段中注册，不要编造工具。本轮实际可调用的工具及联网搜索是否可用，以请求中注册的 tools 为准。select_workflow 只选择本轮工作流提示词并返回是否建议另开 Explanation；list_sources 查看当前材料；get_overview 读取已初始化的概览；read_source 回读指定 PDF 页并取得 rN 引用编号。organize_output 用于从取材切到正式输出；continue_output 用于留在正式输出并继续当前片段；continue_collection 用于回到取材准备下一片段。正式输出只能使用 read_source 结果或输入中实际提供的 rN/wN 引用编号，不要猜测编号。"
}
