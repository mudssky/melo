use sea_orm_migration::prelude::*;

/// 初始数据库迁移。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    /// 创建 Phase 1 所需的初始表结构与索引。
    ///
    /// # 参数
    /// - `manager`：迁移管理器
    ///
    /// # 返回值
    /// - `Result<(), DbErr>`：迁移结果
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_table(artists_table()).await?;
        manager.create_table(albums_table()).await?;
        manager.create_table(songs_table()).await?;
        manager.create_table(playlists_table()).await?;
        manager.create_table(playlist_entries_table()).await?;
        manager.create_table(artwork_refs_table()).await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_songs_artist_id")
                    .table(Songs::Table)
                    .col(Songs::ArtistId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_songs_album_track")
                    .table(Songs::Table)
                    .col(Songs::AlbumId)
                    .col(Songs::DiscNo)
                    .col(Songs::TrackNo)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_artists_search_name")
                    .table(Artists::Table)
                    .col(Artists::SearchName)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_albums_artist_title")
                    .table(Albums::Table)
                    .col(Albums::AlbumArtistId)
                    .col(Albums::Title)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_playlist_entries_position")
                    .table(PlaylistEntries::Table)
                    .col(PlaylistEntries::PlaylistId)
                    .col(PlaylistEntries::Position)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    /// 回滚初始表结构。
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
                    .table(ArtworkRefs::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .if_exists()
                    .table(PlaylistEntries::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().if_exists().table(Playlists::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().if_exists().table(Songs::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().if_exists().table(Albums::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().if_exists().table(Artists::Table).to_owned())
            .await?;
        Ok(())
    }
}

/// 生成 `artists` 表定义。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `TableCreateStatement`：建表语句
fn artists_table() -> TableCreateStatement {
    Table::create()
        .if_not_exists()
        .table(Artists::Table)
        .col(pk_auto(Artists::Id))
        .col(string(Artists::Name).not_null())
        .col(string(Artists::SortName))
        .col(string(Artists::SearchName).not_null())
        .col(string(Artists::CreatedAt).not_null())
        .col(string(Artists::UpdatedAt).not_null())
        .to_owned()
}

/// 生成 `albums` 表定义。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `TableCreateStatement`：建表语句
fn albums_table() -> TableCreateStatement {
    Table::create()
        .if_not_exists()
        .table(Albums::Table)
        .col(pk_auto(Albums::Id))
        .col(string(Albums::Title).not_null())
        .col(integer(Albums::AlbumArtistId))
        .col(integer(Albums::Year))
        .col(string(Albums::SourceDir))
        .col(string(Albums::CreatedAt).not_null())
        .col(string(Albums::UpdatedAt).not_null())
        .foreign_key(
            ForeignKey::create()
                .name("fk_albums_artist")
                .from(Albums::Table, Albums::AlbumArtistId)
                .to(Artists::Table, Artists::Id)
                .on_delete(ForeignKeyAction::SetNull),
        )
        .to_owned()
}

/// 生成 `songs` 表定义。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `TableCreateStatement`：建表语句
fn songs_table() -> TableCreateStatement {
    Table::create()
        .if_not_exists()
        .table(Songs::Table)
        .col(pk_auto(Songs::Id))
        .col(string(Songs::Path).not_null().unique_key())
        .col(string(Songs::Title).not_null())
        .col(integer(Songs::ArtistId))
        .col(integer(Songs::AlbumId))
        .col(integer(Songs::TrackNo))
        .col(integer(Songs::DiscNo))
        .col(double(Songs::DurationSeconds))
        .col(string(Songs::Genre))
        .col(string(Songs::Lyrics))
        .col(string(Songs::LyricsSourceKind).not_null())
        .col(string(Songs::LyricsSourcePath))
        .col(string(Songs::LyricsFormat))
        .col(string(Songs::LyricsUpdatedAt))
        .col(string(Songs::Format))
        .col(big_integer(Songs::Bitrate))
        .col(big_integer(Songs::SampleRate))
        .col(big_integer(Songs::BitDepth))
        .col(big_integer(Songs::Channels))
        .col(big_integer(Songs::FileSize).not_null())
        .col(big_integer(Songs::FileMtime).not_null())
        .col(string(Songs::AddedAt).not_null())
        .col(string(Songs::ScannedAt).not_null())
        .col(string(Songs::OrganizedAt))
        .col(string(Songs::LastOrganizeRule))
        .col(string(Songs::UpdatedAt).not_null())
        .foreign_key(
            ForeignKey::create()
                .name("fk_songs_artist")
                .from(Songs::Table, Songs::ArtistId)
                .to(Artists::Table, Artists::Id)
                .on_delete(ForeignKeyAction::SetNull),
        )
        .foreign_key(
            ForeignKey::create()
                .name("fk_songs_album")
                .from(Songs::Table, Songs::AlbumId)
                .to(Albums::Table, Albums::Id)
                .on_delete(ForeignKeyAction::SetNull),
        )
        .to_owned()
}

/// 生成 `playlists` 表定义。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `TableCreateStatement`：建表语句
fn playlists_table() -> TableCreateStatement {
    Table::create()
        .if_not_exists()
        .table(Playlists::Table)
        .col(pk_auto(Playlists::Id))
        .col(string(Playlists::Name).not_null().unique_key())
        .col(string(Playlists::Description))
        .col(string(Playlists::CreatedAt).not_null())
        .col(string(Playlists::UpdatedAt).not_null())
        .to_owned()
}

/// 生成 `playlist_entries` 表定义。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `TableCreateStatement`：建表语句
fn playlist_entries_table() -> TableCreateStatement {
    Table::create()
        .if_not_exists()
        .table(PlaylistEntries::Table)
        .col(pk_auto(PlaylistEntries::Id))
        .col(big_integer(PlaylistEntries::PlaylistId).not_null())
        .col(big_integer(PlaylistEntries::SongId).not_null())
        .col(big_integer(PlaylistEntries::Position).not_null())
        .col(string(PlaylistEntries::AddedAt).not_null())
        .foreign_key(
            ForeignKey::create()
                .name("fk_playlist_entries_playlist")
                .from(PlaylistEntries::Table, PlaylistEntries::PlaylistId)
                .to(Playlists::Table, Playlists::Id)
                .on_delete(ForeignKeyAction::Cascade),
        )
        .foreign_key(
            ForeignKey::create()
                .name("fk_playlist_entries_song")
                .from(PlaylistEntries::Table, PlaylistEntries::SongId)
                .to(Songs::Table, Songs::Id)
                .on_delete(ForeignKeyAction::Cascade),
        )
        .to_owned()
}

/// 生成 `artwork_refs` 表定义。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `TableCreateStatement`：建表语句
fn artwork_refs_table() -> TableCreateStatement {
    Table::create()
        .if_not_exists()
        .table(ArtworkRefs::Table)
        .col(pk_auto(ArtworkRefs::Id))
        .col(string(ArtworkRefs::OwnerKind).not_null())
        .col(big_integer(ArtworkRefs::OwnerId).not_null())
        .col(string(ArtworkRefs::SourceKind).not_null())
        .col(string(ArtworkRefs::SourcePath))
        .col(big_integer(ArtworkRefs::EmbeddedSongId))
        .col(string(ArtworkRefs::Mime))
        .col(string(ArtworkRefs::CachePath))
        .col(string(ArtworkRefs::Hash))
        .col(string(ArtworkRefs::UpdatedAt).not_null())
        .foreign_key(
            ForeignKey::create()
                .name("fk_artwork_refs_embedded_song")
                .from(ArtworkRefs::Table, ArtworkRefs::EmbeddedSongId)
                .to(Songs::Table, Songs::Id)
                .on_delete(ForeignKeyAction::SetNull),
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

/// 生成整型列定义。
///
/// # 参数
/// - `column`：列标识
///
/// # 返回值
/// - `ColumnDef`：列定义
fn integer<T>(column: T) -> ColumnDef
where
    T: IntoIden,
{
    ColumnDef::new(column).integer().to_owned()
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
enum Artists {
    Table,
    Id,
    Name,
    SortName,
    SearchName,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Albums {
    Table,
    Id,
    Title,
    AlbumArtistId,
    Year,
    SourceDir,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Songs {
    Table,
    Id,
    Path,
    Title,
    ArtistId,
    AlbumId,
    TrackNo,
    DiscNo,
    DurationSeconds,
    Genre,
    Lyrics,
    LyricsSourceKind,
    LyricsSourcePath,
    LyricsFormat,
    LyricsUpdatedAt,
    Format,
    Bitrate,
    SampleRate,
    BitDepth,
    Channels,
    FileSize,
    FileMtime,
    AddedAt,
    ScannedAt,
    OrganizedAt,
    LastOrganizeRule,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Playlists {
    Table,
    Id,
    Name,
    Description,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum PlaylistEntries {
    Table,
    Id,
    PlaylistId,
    SongId,
    Position,
    AddedAt,
}

#[derive(DeriveIden)]
enum ArtworkRefs {
    Table,
    Id,
    OwnerKind,
    OwnerId,
    SourceKind,
    SourcePath,
    EmbeddedSongId,
    Mime,
    CachePath,
    Hash,
    UpdatedAt,
}
