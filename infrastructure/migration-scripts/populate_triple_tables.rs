use std::str::FromStr;
use sqlx::{PgPool, Row};
use models::types::U256Wrapper;
use alloy::primitives::U256;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Database connection - update with your connection string
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/intuition".to_string());
    
    let pool = PgPool::connect(&database_url).await?;
    
    println!("Starting migration to populate triple tables...");
    
    // Get all triples
    let triples = sqlx::query("SELECT term_id, counter_term_id, block_number FROM triple")
        .fetch_all(&pool)
        .await?;
    
    println!("Found {} triples to process", triples.len());
    
    for (i, triple_row) in triples.iter().enumerate() {
        let term_id: String = triple_row.get("term_id");
        let counter_term_id: String = triple_row.get("counter_term_id");
        let block_number: String = triple_row.get("block_number");
        
        let term_id_u256 = U256Wrapper::from_str(&term_id)?;
        let counter_term_id_u256 = U256Wrapper::from_str(&counter_term_id)?;
        let block_number_u256 = U256Wrapper::from_str(&block_number)?;
        
        println!("Processing triple {}/{}: term_id={}", i + 1, triples.len(), term_id);
        
        // Get latest share price changes for both term_id and counter_term_id
        let share_price_changes = sqlx::query(
            r#"
            SELECT DISTINCT ON (term_id, curve_id) 
                term_id, curve_id, total_shares, total_assets, share_price
            FROM share_price_change 
            WHERE term_id = $1 OR term_id = $2
            ORDER BY term_id, curve_id, updated_at DESC
            "#
        )
        .bind(&term_id)
        .bind(&counter_term_id)
        .fetch_all(&pool)
        .await?;
        
        // Calculate aggregates
        let mut total_shares = U256Wrapper::default();
        let mut total_assets = U256Wrapper::default();
        let mut total_market_cap = U256Wrapper::default();
        
        for spc_row in &share_price_changes {
            let shares: String = spc_row.get("total_shares");
            let assets: String = spc_row.get("total_assets");
            let price: String = spc_row.get("share_price");
            
            let shares_u256 = U256Wrapper::from_str(&shares)?;
            let assets_u256 = U256Wrapper::from_str(&assets)?;
            let price_u256 = U256Wrapper::from_str(&price)?;
            
            total_shares = total_shares + shares_u256;
            total_assets = total_assets + assets_u256;
            
            // Calculate market cap: (total_shares * share_price) / 10^18
            let market_cap = (shares_u256 * price_u256) / U256Wrapper::from(U256::from(10).pow(U256::from(18)));
            total_market_cap = total_market_cap + market_cap;
        }
        
        // Get position count for curve_id = 1
        let position_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) 
            FROM position 
            WHERE (term_id = $1 OR term_id = $2) AND curve_id = 1
            "#
        )
        .bind(&term_id)
        .bind(&counter_term_id)
        .fetch_one(&pool)
        .await?;
        
        // Insert into triple_term
        sqlx::query(
            r#"
            INSERT INTO triple_term (term_id, total_assets, total_market_cap, updated_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (term_id) DO UPDATE SET
                total_assets = EXCLUDED.total_assets,
                total_market_cap = EXCLUDED.total_market_cap,
                updated_at = NOW()
            "#
        )
        .bind(total_assets.to_big_decimal()?)
        .bind(total_market_cap.to_big_decimal()?)
        .bind(term_id_u256.to_big_decimal()?)
        .execute(&pool)
        .await?;
        
        // Insert into triple_vault for curve_id = 1
        sqlx::query(
            r#"
            INSERT INTO triple_vault (term_id, curve_id, total_shares, total_assets, position_count, market_cap, block_number, log_index, updated_at)
            VALUES ($1, 1, $2, $3, $4, $5, $6, 0, NOW())
            ON CONFLICT (term_id, curve_id) DO UPDATE SET
                total_shares = EXCLUDED.total_shares,
                total_assets = EXCLUDED.total_assets,
                position_count = EXCLUDED.position_count,
                market_cap = EXCLUDED.market_cap,
                block_number = EXCLUDED.block_number,
                log_index = EXCLUDED.log_index,
                updated_at = NOW()
            "#
        )
        .bind(term_id_u256.to_big_decimal()?)
        .bind(total_shares.to_big_decimal()?)
        .bind(total_assets.to_big_decimal()?)
        .bind(position_count)
        .bind(total_market_cap.to_big_decimal()?)
        .bind(block_number_u256.to_big_decimal()?)
        .execute(&pool)
        .await?;
    }
    
    println!("Migration completed successfully!");
    
    // Print summary
    let triple_term_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM triple_term")
        .fetch_one(&pool)
        .await?;
    
    let triple_vault_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM triple_vault")
        .fetch_one(&pool)
        .await?;
    
    println!("Summary:");
    println!("  - triple_term records: {}", triple_term_count);
    println!("  - triple_vault records: {}", triple_vault_count);
    
    Ok(())
} 