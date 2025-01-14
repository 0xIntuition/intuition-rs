use chrono::{DateTime, Utc};
use macon::Builder;
use models::types::U256Wrapper;
use sqlx::PgPool;

#[derive(Builder)]
pub struct SharePrice {
    pub id: U256Wrapper,
    pub block_number: U256Wrapper,
    pub share_price: U256Wrapper,
}

pub struct SharePricePresenter {
    pub id: U256Wrapper,
    pub block_number: U256Wrapper,
    pub share_price: U256Wrapper,
    pub updated_at: DateTime<Utc>,
}

impl SharePrice {
    pub async fn insert(&self, db: &PgPool, schema: &str) -> Result<(), sqlx::Error> {
        let query = format!(
            "INSERT INTO {}.share_price (id, block_number, share_price) VALUES ($1, $2, $3)",
            schema,
        );

        sqlx::query(&query)
            .bind(&self.id)
            .bind(&self.block_number)
            .bind(&self.share_price)
            .execute(db)
            .await?;
        Ok(())
    }
}
