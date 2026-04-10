use sea_orm::entity::prelude::*;

/// 播放会话头实体模型。
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "player_sessions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub playback_state: String,
    pub queue_index: Option<i64>,
    pub position_seconds: Option<f64>,
    pub updated_at: String,
}

/// 播放会话头关联定义。
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
