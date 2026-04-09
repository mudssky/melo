use sea_orm_migration::prelude::*;

/// Melo 的数据库迁移器。
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    /// 返回需要执行的迁移列表。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Vec<Box<dyn MigrationTrait>>`：按顺序执行的迁移集合
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(
            crate::core::db::migrations::m20260410_000001_initial::Migration,
        )]
    }
}
