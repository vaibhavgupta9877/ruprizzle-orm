use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "followers")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub follower_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub followee_id: i64,
    pub created_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::FollowerId",
        to = "super::user::Column::Id"
    )]
    Follower,
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::FolloweeId",
        to = "super::user::Column::Id"
    )]
    Followee,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Follower.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
