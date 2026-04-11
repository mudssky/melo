use sea_orm_migration::prelude::*;

/// 临时歌单元数据迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    /// 为 `playlists` 表增加临时歌单生命周期字段。
    ///
    /// # 参数
    /// - `manager`：迁移管理器
    ///
    /// # 返回值
    /// - `Result<(), DbErr>`：迁移结果
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        add_column(
            manager,
            ColumnDef::new(Playlists::Kind)
                .string()
                .not_null()
                .default("static")
                .to_owned(),
        )
        .await?;
        add_column(
            manager,
            ColumnDef::new(Playlists::SourceKind).string().to_owned(),
        )
        .await?;
        add_column(
            manager,
            ColumnDef::new(Playlists::SourceKey).string().to_owned(),
        )
        .await?;
        add_column(
            manager,
            ColumnDef::new(Playlists::Visible)
                .boolean()
                .not_null()
                .default(true)
                .to_owned(),
        )
        .await?;
        add_column(
            manager,
            ColumnDef::new(Playlists::ExpiresAt).string().to_owned(),
        )
        .await?;
        add_column(
            manager,
            ColumnDef::new(Playlists::LastActivatedAt)
                .string()
                .to_owned(),
        )
        .await?;
        Ok(())
    }

    /// 回滚 `playlists` 表新增的临时歌单字段。
    ///
    /// # 参数
    /// - `manager`：迁移管理器
    ///
    /// # 返回值
    /// - `Result<(), DbErr>`：回滚结果
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_column(manager, Playlists::LastActivatedAt).await?;
        drop_column(manager, Playlists::ExpiresAt).await?;
        drop_column(manager, Playlists::Visible).await?;
        drop_column(manager, Playlists::SourceKey).await?;
        drop_column(manager, Playlists::SourceKind).await?;
        drop_column(manager, Playlists::Kind).await?;
        Ok(())
    }
}

/// 为 `playlists` 表追加一列。
///
/// # 参数
/// - `manager`：迁移管理器
/// - `column`：列定义
///
/// # 返回值
/// - `Result<(), DbErr>`：追加结果
async fn add_column(manager: &SchemaManager<'_>, column: ColumnDef) -> Result<(), DbErr> {
    manager
        .alter_table(
            Table::alter()
                .table(Playlists::Table)
                .add_column(column)
                .to_owned(),
        )
        .await
}

/// 从 `playlists` 表删除一列。
///
/// # 参数
/// - `manager`：迁移管理器
/// - `column`：列标识
///
/// # 返回值
/// - `Result<(), DbErr>`：删除结果
async fn drop_column(manager: &SchemaManager<'_>, column: Playlists) -> Result<(), DbErr> {
    manager
        .alter_table(
            Table::alter()
                .table(Playlists::Table)
                .drop_column(column)
                .to_owned(),
        )
        .await
}

#[derive(DeriveIden)]
enum Playlists {
    Table,
    Kind,
    SourceKind,
    SourceKey,
    Visible,
    ExpiresAt,
    LastActivatedAt,
}
