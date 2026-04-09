use sea_orm::entity::prelude::*;

/// 歌曲实体模型。
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "songs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub path: String,
    pub title: String,
    pub artist_id: Option<i64>,
    pub album_id: Option<i64>,
    pub track_no: Option<i64>,
    pub disc_no: Option<i64>,
    pub duration_seconds: Option<f64>,
    pub genre: Option<String>,
    pub lyrics: Option<String>,
    pub lyrics_source_kind: String,
    pub lyrics_source_path: Option<String>,
    pub lyrics_format: Option<String>,
    pub lyrics_updated_at: Option<String>,
    pub format: Option<String>,
    pub bitrate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub bit_depth: Option<i64>,
    pub channels: Option<i64>,
    pub file_size: i64,
    pub file_mtime: i64,
    pub added_at: String,
    pub scanned_at: String,
    pub organized_at: Option<String>,
    pub last_organize_rule: Option<String>,
    pub updated_at: String,
}

/// 歌曲关联定义。
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::artists::Entity",
        from = "Column::ArtistId",
        to = "super::artists::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    Artist,
    #[sea_orm(
        belongs_to = "super::albums::Entity",
        from = "Column::AlbumId",
        to = "super::albums::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    Album,
}

impl ActiveModelBehavior for ActiveModel {}
