use crate::delivery::{Delivery, DeliveryStatus};

#[derive(sqlx::FromRow)]
struct DeliveryRow {
    id: i64,
    event: String,
    attempts: i32,
    status: String,
}

impl TryFrom<DeliveryRow> for Delivery {
    type Error = String;

    fn try_from(row: DeliveryRow) -> Result<Self, Self::Error> {
        let id = u64::try_from(row.id)
            .map_err(|error| format!("invalid delivery ID {}: {error}", row.id))?;
        let status = row.status.parse::<DeliveryStatus>()?;

        Ok(Delivery {
            id,
            event: row.event,
            attempts: row.attempts,
            status,
        })
    }
}

#[derive(Debug)]
pub enum DbError {
    BlankEvent,
    Database(sqlx::Error),
    InvalidData(String),
    InvalidId(u64),
    BlankIdempotencyKey,
    IdempotencyConflict,
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::BlankEvent => f.write_str("event must not be blank"),
            DbError::Database(error) => write!(f, "{error}"),
            DbError::InvalidData(message) => f.write_str(message),
            DbError::InvalidId(id) => write!(f, "delivery ID {id} exceeds the supported range"),
            DbError::BlankIdempotencyKey => f.write_str("idempotency key must not be blank"),
            DbError::IdempotencyConflict => {
                f.write_str("idempotency key was already used with a different event")
            }
        }
    }
}

impl std::error::Error for DbError {}

pub async fn list_deliveries(pool: &sqlx::PgPool) -> Result<Vec<Delivery>, DbError> {
    let result: Vec<DeliveryRow> =
        sqlx::query_as("SELECT id, event, attempts, status FROM deliveries ORDER BY id")
            .fetch_all(pool)
            .await
            .map_err(DbError::Database)?;

    let mut deliveries: Vec<Delivery> = vec![];
    for row in result {
        deliveries.push(Delivery::try_from(row).map_err(DbError::InvalidData)?);
    }

    Ok(deliveries)
}

pub async fn get_delivery(pool: &sqlx::PgPool, id: u64) -> Result<Option<Delivery>, DbError> {
    let signed_id = i64::try_from(id).map_err(|_| DbError::InvalidId(id))?;
    let row: Option<DeliveryRow> =
        sqlx::query_as("SELECT id, event, attempts, status FROM deliveries WHERE id = $1")
            .bind(signed_id)
            .fetch_optional(pool)
            .await
            .map_err(DbError::Database)?;

    match row {
        Some(row) => Ok(Some(Delivery::try_from(row).map_err(DbError::InvalidData)?)),
        None => Ok(None),
    }
}

pub async fn create_delivery(pool: &sqlx::PgPool, event: &str) -> Result<Delivery, DbError> {
    if event.trim().is_empty() {
        return Err(DbError::BlankEvent);
    }

    let row: DeliveryRow = sqlx::query_as(
        "INSERT INTO deliveries (event) VALUES ($1) RETURNING id, event, attempts, status",
    )
    .bind(event)
    .fetch_one(pool)
    .await
    .map_err(DbError::Database)?;

    Delivery::try_from(row).map_err(DbError::InvalidData)
}

pub async fn create_delivery_with_key(
    pool: &sqlx::PgPool,
    event: &str,
    key: &str,
) -> Result<Delivery, DbError> {
    if event.trim().is_empty() {
        return Err(DbError::BlankEvent);
    }

    if key.trim().is_empty() {
        return Err(DbError::BlankIdempotencyKey);
    }

    let mut transaction = pool.begin().await.map_err(DbError::Database)?;

    let row: DeliveryRow = sqlx::query_as(
        "INSERT INTO deliveries (event) VALUES ($1) RETURNING id, event, attempts, status",
    )
    .bind(event)
    .fetch_one(&mut *transaction)
    .await
    .map_err(DbError::Database)?;

    let rows_affeced = sqlx::query("INSERT INTO idempotency_keys (key, delivery_id) VALUES ($1, $2) ON CONFLICT (key) DO NOTHING")
        .bind(key)
        .bind(row.id)
        .execute(&mut *transaction)
        .await
        .map_err(DbError::Database)?
        .rows_affected();

    if rows_affeced == 0 {
        transaction.rollback().await.map_err(DbError::Database)?;

        let row: DeliveryRow = sqlx::query_as(
            "SELECT d.id, d.event, d.attempts, d.status FROM deliveries AS d JOIN idempotency_keys AS k ON k.delivery_id = d.id WHERE k.key = $1",
        )
        .bind(key)
        .fetch_one(pool)
        .await
        .map_err(DbError::Database)?;

        if row.event == event {
            return Delivery::try_from(row).map_err(DbError::InvalidData);
        } else {
            return Err(DbError::IdempotencyConflict);
        }
    }

    let delivery = Delivery::try_from(row).map_err(DbError::InvalidData)?;

    transaction.commit().await.map_err(DbError::Database)?;

    Ok(delivery)
}

#[cfg(test)]
mod tests {
    use crate::{
        db::DeliveryRow,
        delivery::{Delivery, DeliveryStatus},
    };

    #[test]
    fn valid_row_converts_to_delivery() {
        let row = DeliveryRow {
            id: 7,
            event: String::from("order.created"),
            attempts: 2,
            status: String::from("succeeded"),
        };
        let delivery = Delivery::try_from(row).unwrap();

        assert_eq!(delivery.id, 7);
        assert_eq!(delivery.event, "order.created");
        assert_eq!(delivery.attempts, 2);
        assert!(matches!(delivery.status, DeliveryStatus::Succeeded));
    }

    #[test]
    fn negative_id_is_rejected() {
        let row = DeliveryRow {
            id: -1,
            event: String::from("order.created"),
            attempts: 2,
            status: String::from("succeeded"),
        };
        let error = Delivery::try_from(row).unwrap_err();

        assert!(error.contains("invalid delivery ID"));
        assert!(error.contains("-1"));
    }

    #[test]
    fn unknown_status_is_rejected() {
        let row = DeliveryRow {
            id: 7,
            event: String::from("order.created"),
            attempts: 2,
            status: String::from("queued"),
        };
        let error = Delivery::try_from(row).unwrap_err();

        assert_eq!(error, "unknown delivery status: queued");
    }
}
