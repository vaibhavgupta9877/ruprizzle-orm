use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "tags")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        has_many = "super::post_tag::Entity",
        from = "Column::Id",
        to = "super::post_tag::Column::TagId"
    )]
    PostTag,
}

impl Related<super::post_tag::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PostTag.def()
    }
}

impl Related<super::post::Entity> for Entity {
    fn to() -> RelationDef {
        super::post_tag::Relation::Post.def()
    }

    fn via() -> Option<RelationDef> {
        Some(super::post_tag::Relation::Tag.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}
