use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "offline_terms")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub rowid: i64,
    pub expression: String,
    pub reading: String,
    pub definition: String,
    pub pitch_accent: String,
    pub dict_name: String,
    pub score: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
