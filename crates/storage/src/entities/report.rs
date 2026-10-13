use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "clickman_reports")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub project_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub key: String,
    pub result: Json,
    pub computed_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
