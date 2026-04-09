use sea_orm::{Database, DatabaseConnection};

use crate::core::config::settings::Settings;
use crate::core::error::{MeloError, MeloResult};

/// 生成 SQLite 的 SeaORM 连接字符串。
///
/// # 参数
/// - `settings`：全局配置
///
/// # 返回值
/// - `String`：可供 `SeaORM` 使用的 SQLite URL
pub fn database_url(settings: &Settings) -> String {
    let path = settings.database.path.as_str().replace('\\', "/");
    format!("sqlite://{path}?mode=rwc")
}

/// 打开 `SeaORM` 数据库连接。
///
/// # 参数
/// - `settings`：全局配置
///
/// # 返回值
/// - `MeloResult<DatabaseConnection>`：异步数据库连接
pub async fn connect(settings: &Settings) -> MeloResult<DatabaseConnection> {
    Database::connect(database_url(settings))
        .await
        .map_err(|err| MeloError::Message(err.to_string()))
}
