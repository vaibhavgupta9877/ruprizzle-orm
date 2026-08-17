use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "posts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub author_id: i64,
    pub category_id: i64,
    pub title: String,
    pub published_at: i64,
    pub views: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::AuthorId",
        to = "super::user::Column::Id"
    )]
    User,
    #[sea_orm(
        belongs_to = "super::category::Entity",
        from = "Column::CategoryId",
        to = "super::category::Column::Id"
    )]
    Category,
    #[sea_orm(
        has_many = "super::comment::Entity",
        from = "Column::Id",
        to = "super::comment::Column::PostId"
    )]
    Comment,
    #[sea_orm(
        has_many = "super::like::Entity",
        from = "Column::Id",
        to = "super::like::Column::PostId"
    )]
    Like,
    #[sea_orm(
        has_many = "super::post_tag::Entity",
        from = "Column::Id",
        to = "super::post_tag::Column::PostId"
    )]
    PostTag,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl Related<super::category::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Category.def()
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

impl Related<super::post_tag::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PostTag.def()
    }
}

impl Related<super::tag::Entity> for Entity {
    fn to() -> RelationDef {
        super::post_tag::Relation::Tag.def()
    }

    fn via() -> Option<RelationDef> {
        Some(super::post_tag::Relation::Post.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}
