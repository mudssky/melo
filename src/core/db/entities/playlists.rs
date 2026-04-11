use sea_orm::entity::prelude::*;

/// 歌单实体模型。
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "playlists")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub kind: String,
    pub source_kind: Option<String>,
    pub source_key: Option<String>,
    pub visible: bool,
    pub expires_at: Option<String>,
    pub last_activated_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 歌单关联定义。
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
