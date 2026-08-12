use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub email: String,
    pub age: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        has_many = "super::post::Entity",
        from = "Column::Id",
        to = "super::post::Column::AuthorId"
    )]
    Post,
}

impl Related<super::post::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Post.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
