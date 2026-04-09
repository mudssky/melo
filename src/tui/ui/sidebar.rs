/// 返回侧边栏的默认栏目列表。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `Vec<&'static str>`：默认侧边栏栏目
pub fn sections() -> Vec<&'static str> {
    vec!["Songs", "Playlists", "Queue"]
}
