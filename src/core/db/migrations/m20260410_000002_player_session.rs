use sea_orm_migration::prelude::*;

/// 播放会话持久化迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    /// 创建播放会话头表和队列项表。
    ///
    /// # 参数
    /// - `manager`：迁移管理器
    ///
    /// # 返回值
    /// - `Result<(), DbErr>`：迁移结果
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_table(player_sessions_table()).await?;
        manager.create_table(player_session_items_table()).await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_player_session_items_session_position")
                    .table(PlayerSessionItems::Table)
                    .col(PlayerSessionItems::SessionId)
                    .col(PlayerSessionItems::Position)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    /// 回滚播放会话相关表结构。
    ///
    /// # 参数
    /// - `manager`：迁移管理器
    ///
    /// # 返回值
    /// - `Result<(), DbErr>`：回滚结果
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .if_exists()
                    .table(PlayerSessionItems::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .if_exists()
                    .table(PlayerSessions::Table)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

/// 生成 `player_sessions` 表定义。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `TableCreateStatement`：建表语句
fn player_sessions_table() -> TableCreateStatement {
    Table::create()
        .if_not_exists()
        .table(PlayerSessions::Table)
        .col(pk_auto(PlayerSessions::Id))
        .col(string(PlayerSessions::PlaybackState).not_null())
        .col(big_integer(PlayerSessions::QueueIndex))
        .col(double(PlayerSessions::PositionSeconds))
        .col(string(PlayerSessions::UpdatedAt).not_null())
        .to_owned()
}

/// 生成 `player_session_items` 表定义。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `TableCreateStatement`：建表语句
fn player_session_items_table() -> TableCreateStatement {
    Table::create()
        .if_not_exists()
        .table(PlayerSessionItems::Table)
        .col(pk_auto(PlayerSessionItems::Id))
        .col(big_integer(PlayerSessionItems::SessionId).not_null())
        .col(big_integer(PlayerSessionItems::Position).not_null())
        .col(big_integer(PlayerSessionItems::SongId).not_null())
        .col(string(PlayerSessionItems::Path).not_null())
        .col(string(PlayerSessionItems::Title).not_null())
        .col(double(PlayerSessionItems::DurationSeconds))
        .foreign_key(
            ForeignKey::create()
                .name("fk_player_session_items_session")
                .from(PlayerSessionItems::Table, PlayerSessionItems::SessionId)
                .to(PlayerSessions::Table, PlayerSessions::Id)
                .on_delete(ForeignKeyAction::Cascade),
        )
        .to_owned()
}

/// 生成整型自增主键列。
///
/// # 参数
/// - `column`：列标识
///
/// # 返回值
/// - `ColumnDef`：列定义
fn pk_auto<T>(column: T) -> ColumnDef
where
    T: IntoIden,
{
    ColumnDef::new(column)
        .big_integer()
        .not_null()
        .auto_increment()
        .primary_key()
        .to_owned()
}

/// 生成文本列定义。
///
/// # 参数
/// - `column`：列标识
///
/// # 返回值
/// - `ColumnDef`：列定义
fn string<T>(column: T) -> ColumnDef
where
    T: IntoIden,
{
    ColumnDef::new(column).string().to_owned()
}

/// 生成大整型列定义。
///
/// # 参数
/// - `column`：列标识
///
/// # 返回值
/// - `ColumnDef`：列定义
fn big_integer<T>(column: T) -> ColumnDef
where
    T: IntoIden,
{
    ColumnDef::new(column).big_integer().to_owned()
}

/// 生成浮点列定义。
///
/// # 参数
/// - `column`：列标识
///
/// # 返回值
/// - `ColumnDef`：列定义
fn double<T>(column: T) -> ColumnDef
where
    T: IntoIden,
{
    ColumnDef::new(column).double().to_owned()
}

#[derive(DeriveIden)]
enum PlayerSessions {
    Table,
    Id,
    PlaybackState,
    QueueIndex,
    PositionSeconds,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum PlayerSessionItems {
    Table,
    Id,
    SessionId,
    Position,
    SongId,
    Path,
    Title,
    DurationSeconds,
}
