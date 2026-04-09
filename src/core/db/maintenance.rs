use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};

use crate::core::error::{MeloError, MeloResult};

/// 执行 SQLite `VACUUM`。
///
/// # 参数
/// - `path`：数据库路径
///
/// # 返回值
/// - `MeloResult<()>`：执行结果
pub async fn vacuum(path: &std::path::Path) -> MeloResult<()> {
    let database_url = format!(
        "sqlite://{}?mode=rwc",
        path.to_string_lossy().replace('\\', "/")
    );
    let connection = Database::connect(&database_url)
        .await
        .map_err(|err| MeloError::Message(err.to_string()))?;
    connection
        .execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "VACUUM".to_string(),
        ))
        .await
        .map_err(|err| MeloError::Message(err.to_string()))?;
    Ok(())
}

/// 复制数据库文件作为备份。
///
/// # 参数
/// - `path`：源数据库路径
/// - `dest`：目标备份路径
///
/// # 返回值
/// - `MeloResult<()>`：执行结果
pub fn backup(path: &std::path::Path, dest: &std::path::Path) -> MeloResult<()> {
    std::fs::copy(path, dest)
        .map(|_| ())
        .map_err(|err| MeloError::Message(err.to_string()))
}
