use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub email: String,
    pub age: i64,
    pub name: String,
    pub created_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        has_many = "super::post::Entity",
        from = "Column::Id",
        to = "super::post::Column::AuthorId"
    )]
    Post,
    #[sea_orm(
        has_many = "super::comment::Entity",
        from = "Column::Id",
        to = "super::comment::Column::AuthorId"
    )]
    Comment,
    #[sea_orm(
        has_many = "super::like::Entity",
        from = "Column::Id",
        to = "super::like::Column::UserId"
    )]
    Like,
    #[sea_orm(
        has_many = "super::follower::Entity",
        from = "Column::Id",
        to = "super::follower::Column::FolloweeId"
    )]
    Follower,
}

impl Related<super::post::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Post.def()
    }
}

impl Related<super::comment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Comment.def()
    }
}

impl Related<super::like::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Like.def()
    }
}

impl Related<super::follower::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Follower.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
