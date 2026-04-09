/// 返回帮助弹窗的默认文案。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `Vec<&'static str>`：帮助提示列表
pub fn help_lines() -> Vec<&'static str> {
    vec!["Space: Toggle", ">: Next", "<: Previous", "q: Quit"]
}
