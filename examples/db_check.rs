use hookrun::{
    db::{get_delivery, list_deliveries},
    delivery::Delivery,
};
use sqlx::postgres::PgPoolOptions;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let url = env::var("DATABASE_URL").unwrap();
    let pool = PgPoolOptions::new().connect(&url).await?;

    let deliveries = list_deliveries(&pool).await?;

    println!("Deliveries count: {}", deliveries.len());

    for Delivery {
        id,
        event,
        attempts,
        status,
    } in deliveries
    {
        println!("Delivery {id}: {event}, attempts {attempts}, status {status}");
    }

    for id in [1, 999] {
        match get_delivery(&pool, id).await? {
            Some(Delivery {
                id,
                event,
                attempts,
                status,
            }) => println!("Delivery {id}: {event}, attempts {attempts}, status {status}"),
            None => println!("delivery {id} not found"),
        };
    }

    Ok(())
}
