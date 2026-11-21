use hookrun::{
    db::{DbError, create_delivery, create_delivery_with_key, get_delivery, list_deliveries},
    delivery::{Delivery, DeliveryStatus},
};
use std::assert_matches;

#[sqlx::test]
async fn created_delivery_can_be_fetched_by_id(pool: sqlx::PgPool) {
    let empty = list_deliveries(&pool).await.unwrap();
    assert_eq!(empty.len(), 0);

    let Delivery {
        id,
        event,
        attempts,
        status,
    } = create_delivery(&pool, "order.created").await.unwrap();
    assert!(id > 0);
    assert_eq!(event, "order.created");
    assert_eq!(attempts, 0);
    assert_matches!(status, DeliveryStatus::Pending);

    let found = get_delivery(&pool, id)
        .await
        .unwrap()
        .expect("expect a delivery");

    assert_eq!(id, found.id);
    assert_eq!(event, found.event);
    assert_eq!(attempts, found.attempts);
    assert_eq!(status, found.status);
}

#[sqlx::test]
async fn blank_event_is_rejected_without_persisting_delivery(pool: sqlx::PgPool) {
    let error = create_delivery(&pool, " ").await.unwrap_err();
    assert!(matches!(error, DbError::BlankEvent));

    let empty = list_deliveries(&pool).await.unwrap();
    assert_eq!(empty.len(), 0);
}

#[sqlx::test]
async fn missing_delivery_returns_none(pool: sqlx::PgPool) {
    let option = get_delivery(&pool, 1).await.unwrap();
    assert!(option.is_none())
}

#[sqlx::test]
async fn oversized_delivery_id_is_rejected(pool: sqlx::PgPool) {
    let id = u64::MAX;
    let error = get_delivery(&pool, id).await.unwrap_err();
    assert_matches!(error, DbError::InvalidId(u64::MAX));
}

#[sqlx::test]
async fn create_delivery_with_key_persists_delivery_and_key(pool: sqlx::PgPool) {
    let key = "create-order-123";

    let created = create_delivery_with_key(&pool, "order.created", key)
        .await
        .unwrap();
    assert!(created.id > 0);
    assert_eq!(created.event, "order.created");
    assert_eq!(created.attempts, 0);
    assert_matches!(created.status, DeliveryStatus::Pending);

    let mapped_id: i64 =
        sqlx::query_scalar("SELECT delivery_id FROM idempotency_keys WHERE key = $1")
            .bind(key)
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(mapped_id, i64::try_from(created.id).unwrap());

    let Delivery {
        id,
        event,
        attempts,
        status,
    } = get_delivery(&pool, created.id)
        .await
        .unwrap()
        .expect("expect a delivery");

    assert_eq!(created.id, id);
    assert_eq!(created.event, event);
    assert_eq!(created.attempts, attempts);
    assert_eq!(created.status, status);
}

#[sqlx::test]
async fn duplicate_key_rolls_back_new_delivery(pool: sqlx::PgPool) {
    let key = "duplicate-key";

    let created = create_delivery_with_key(&pool, "order.created", key)
        .await
        .unwrap();

    let error = create_delivery_with_key(&pool, "order.refunded", key)
        .await
        .unwrap_err();
    assert_matches!(error, DbError::IdempotencyConflict);

    let deliveries = list_deliveries(&pool).await.unwrap();
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].id, created.id);
    assert_eq!(deliveries[0].event, created.event);
    assert_eq!(deliveries[0].attempts, created.attempts);
    assert_eq!(deliveries[0].status, created.status);

    let mapped_id: i64 =
        sqlx::query_scalar("SELECT delivery_id FROM idempotency_keys WHERE key = $1")
            .bind(key)
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(mapped_id, i64::try_from(created.id).unwrap());
}

#[sqlx::test]
async fn same_key_and_event_returns_original_delivery(pool: sqlx::PgPool) {
    let key = "duplicate-key";

    let created = create_delivery_with_key(&pool, "order.created", key)
        .await
        .unwrap();

    let found = create_delivery_with_key(&pool, "order.created", key)
        .await
        .unwrap();

    assert_eq!(created.id, found.id);
    assert_eq!(created.event, found.event);
    assert_eq!(created.attempts, found.attempts);
    assert_eq!(created.status, found.status);

    let deliveries = list_deliveries(&pool).await.unwrap();
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].id, created.id);
    assert_eq!(deliveries[0].event, created.event);
    assert_eq!(deliveries[0].attempts, created.attempts);
    assert_eq!(deliveries[0].status, created.status);
}

#[sqlx::test]
async fn concurrent_same_key_returns_one_delivery(pool: sqlx::PgPool) {
    let key = "duplicate-key";

    let create_pool = pool.clone();
    let duplicate_pool = pool.clone();

    let create_task = tokio::spawn(async move {
        create_delivery_with_key(&create_pool.clone(), "order.created", key)
            .await
            .unwrap()
    });

    let duplicate_task = tokio::spawn(async move {
        create_delivery_with_key(&duplicate_pool.clone(), "order.created", key)
            .await
            .unwrap()
    });

    let created = create_task.await.unwrap();
    let found = duplicate_task.await.unwrap();
    assert_eq!(created.id, found.id);
    assert_eq!(created.event, found.event);
    assert_eq!(created.attempts, found.attempts);
    assert_eq!(created.status, found.status);

    let deliveries = list_deliveries(&pool).await.unwrap();
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].id, created.id);
    assert_eq!(deliveries[0].event, created.event);
    assert_eq!(deliveries[0].attempts, created.attempts);
    assert_eq!(deliveries[0].status, created.status);

    let mapped_ids: Vec<i64> =
        sqlx::query_scalar("SELECT delivery_id FROM idempotency_keys WHERE key = $1")
            .bind(key)
            .fetch_all(&pool)
            .await
            .unwrap();

    assert_eq!(mapped_ids.len(), 1);
    assert_eq!(mapped_ids[0], i64::try_from(created.id).unwrap());
}
