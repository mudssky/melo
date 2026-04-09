use rusqlite::Connection;

use crate::core::config::settings::Settings;
use crate::core::error::{MeloError, MeloResult};

/// 打开 SQLite 连接。
///
/// # 参数
/// - `settings`：全局配置
///
/// # 返回
/// - `MeloResult<Connection>`：打开后的数据库连接
pub fn connect(settings: &Settings) -> MeloResult<Connection> {
    Connection::open(settings.database.path.as_std_path())
        .map_err(|err| MeloError::Message(err.to_string()))
}
