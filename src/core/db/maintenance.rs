use crate::core::error::{MeloError, MeloResult};

/// 执行 SQLite `VACUUM`。
///
/// # 参数
/// - `path`：数据库路径
///
/// # 返回
/// - `MeloResult<()>`：执行结果
pub fn vacuum(path: &std::path::Path) -> MeloResult<()> {
    let conn =
        rusqlite::Connection::open(path).map_err(|err| MeloError::Message(err.to_string()))?;
    conn.execute_batch("VACUUM;")
        .map_err(|err| MeloError::Message(err.to_string()))?;
    Ok(())
}

/// 复制数据库文件作为备份。
///
/// # 参数
/// - `path`：源数据库路径
/// - `dest`：目标备份路径
///
/// # 返回
/// - `MeloResult<()>`：执行结果
pub fn backup(path: &std::path::Path, dest: &std::path::Path) -> MeloResult<()> {
    std::fs::copy(path, dest)
        .map(|_| ())
        .map_err(|err| MeloError::Message(err.to_string()))
}
