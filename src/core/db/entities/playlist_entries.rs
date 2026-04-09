use sea_orm::entity::prelude::*;

/// 歌单成员实体模型。
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "playlist_entries")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub playlist_id: i64,
    pub song_id: i64,
    pub position: i64,
    pub added_at: String,
}

/// 歌单成员关联定义。
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::playlists::Entity",
        from = "Column::PlaylistId",
        to = "super::playlists::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Playlist,
    #[sea_orm(
        belongs_to = "super::songs::Entity",
        from = "Column::SongId",
        to = "super::songs::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Song,
}

impl ActiveModelBehavior for ActiveModel {}
