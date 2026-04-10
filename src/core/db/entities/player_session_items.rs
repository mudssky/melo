use sea_orm::entity::prelude::*;

/// 播放会话队列项实体模型。
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "player_session_items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub session_id: i64,
    pub position: i64,
    pub song_id: i64,
    pub path: String,
    pub title: String,
    pub duration_seconds: Option<f64>,
}

/// 播放会话队列项关联定义。
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::player_sessions::Entity",
        from = "Column::SessionId",
        to = "super::player_sessions::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Session,
}

impl ActiveModelBehavior for ActiveModel {}
