use sea_orm::entity::prelude::*;

/// 封面引用实体模型。
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "artwork_refs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub owner_kind: String,
    pub owner_id: i64,
    pub source_kind: String,
    pub source_path: Option<String>,
    pub embedded_song_id: Option<i64>,
    pub mime: Option<String>,
    pub cache_path: Option<String>,
    pub hash: Option<String>,
    pub updated_at: String,
}

/// 封面引用关联定义。
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::songs::Entity",
        from = "Column::EmbeddedSongId",
        to = "super::songs::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    EmbeddedSong,
}

impl ActiveModelBehavior for ActiveModel {}
