use sea_orm::entity::prelude::*;

/// 艺术家实体模型。
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "artists")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub sort_name: Option<String>,
    pub search_name: String,
    pub created_at: String,
    pub updated_at: String,
}

/// 艺术家关联定义。
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
