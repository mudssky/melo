use sea_orm::entity::prelude::*;

/// 专辑实体模型。
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "albums")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub title: String,
    pub album_artist_id: Option<i64>,
    pub year: Option<i32>,
    pub source_dir: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 专辑关联定义。
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::artists::Entity",
        from = "Column::AlbumArtistId",
        to = "super::artists::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    Artist,
}

impl ActiveModelBehavior for ActiveModel {}
