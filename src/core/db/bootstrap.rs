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
            fs::create_dir_all(parent).map_err(|err| map_prepare_error("create_dir", err))?;
        }

        let connection = connect(self.settings)
            .await
            .map_err(|err| map_prepare_error("connect", err))?;
        crate::core::db::migrator::Migrator::up(&connection, None)
            .await
            .map_err(|err| map_prepare_error("migrate", err))?;
        Ok(())
    }

    /// 为 daemon 与 CLI 运行时准备数据库。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<()>`：准备结果
    pub async fn prepare_runtime_database(&self) -> MeloResult<()> {
        self.init().await
    }
}

/// 统一包装数据库准备阶段错误。
///
/// # 参数
/// - `stage`：失败阶段
/// - `err`：底层错误
///
/// # 返回值
/// - `MeloError`：带稳定前缀的准备错误
fn map_prepare_error(stage: &str, err: impl std::fmt::Display) -> MeloError {
    MeloError::Message(format!("failed to prepare database: {stage}: {err}"))
}
