use std::fs;

use sea_orm_migration::MigratorTrait;

use crate::core::config::settings::Settings;
use crate::core::db::connection::connect;
use crate::core::error::{MeloError, MeloResult};

/// 数据库初始化器，负责确保迁移按顺序执行。
pub struct DatabaseBootstrap<'a> {
    settings: &'a Settings,
}

impl<'a> DatabaseBootstrap<'a> {
    /// 创建新的数据库初始化器。
    ///
    /// # 参数
    /// - `settings`：全局配置
    ///
    /// # 返回值
    /// - `Self`：数据库初始化器
    pub fn new(settings: &'a Settings) -> Self {
        Self { settings }
    }

    /// 初始化数据库并执行所有未完成的迁移。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<()>`：初始化结果
    pub async fn init(&self) -> MeloResult<()> {
        let path = self.settings.database.path.as_std_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| MeloError::Message(err.to_string()))?;
        }

        let connection = connect(self.settings).await?;
        crate::core::db::migrator::Migrator::up(&connection, None)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;
        Ok(())
    }
}
