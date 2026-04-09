pub mod bootstrap;
pub mod connection;
pub mod entities;
pub mod maintenance;
pub mod migrations;
pub mod migrator;

/// 生成统一使用的文本时间戳。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `String`：基于 Unix 时间戳秒数的文本表示
pub fn now_text() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}
