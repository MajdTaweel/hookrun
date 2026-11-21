use hookrun::{db::create_delivery, delivery::Delivery};
use sqlx::postgres::PgPoolOptions;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let url = env::var("DATABASE_URL").unwrap();
    let pool = PgPoolOptions::new().connect(&url).await?;

    let Delivery {
        id,
        event,
        attempts,
        status,
    } = create_delivery(&pool, "order.created").await?;
    println!("Delivery {id}: {event}, attempts {attempts}, status {status}");

    Ok(())
}
