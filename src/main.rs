mod api_error;
mod deliveries;
mod echo;
mod health;

use axum::{
    Router,
    routing::{get, post},
};
use hookrun::delivery::{Delivery, DeliveryStatus, find_delivery, mark_failed, mark_succeeded};
use sqlx::postgres::PgPoolOptions;

#[derive(Clone)]
struct AppState {
    pool: sqlx::PgPool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    run_demo();

    let url = std::env::var("DATABASE_URL")?;
    let pool = PgPoolOptions::new().connect(&url).await?;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, app(pool)).await?;

    Ok(())
}

fn app(pool: sqlx::PgPool) -> Router {
    let app_state = AppState { pool };

    Router::new()
        .route("/health", get(health::health))
        .route("/echo", post(echo::echo))
        .route(
            "/deliveries",
            get(deliveries::list_deliveries).post(deliveries::create_delivery),
        )
        .route("/deliveries/{id}", get(deliveries::get_delivery))
        .with_state(app_state)
}

fn run_demo() {
    let mut successful_delivery = Delivery {
        id: 1,
        event: String::from("order.created"),
        attempts: 2,
        status: DeliveryStatus::Pending,
    };

    print_attempt(&successful_delivery);
    successful_delivery.attempts += 1;

    match mark_succeeded(&mut successful_delivery) {
        Ok(_) => print_attempt(&successful_delivery),
        Err(error) => println!("{error}"),
    };

    match mark_succeeded(&mut successful_delivery) {
        Ok(_) => print_attempt(&successful_delivery),
        Err(error) => println!("{error}"),
    };

    print_attempt(&successful_delivery);

    let mut failed_delivery = Delivery {
        id: 2,
        event: String::from("order.canceled"),
        attempts: 1,
        status: DeliveryStatus::Pending,
    };

    print_attempt(&failed_delivery);
    failed_delivery.attempts += 1;

    match mark_failed(&mut failed_delivery) {
        Ok(_) => print_attempt(&failed_delivery),
        Err(error) => println!("{error}"),
    };

    match mark_succeeded(&mut failed_delivery) {
        Ok(_) => print_attempt(&failed_delivery),
        Err(error) => println!("{error}"),
    };

    print_attempt(&failed_delivery);

    let deliveries = vec![successful_delivery, failed_delivery];

    println!("Looking for deliveries...");

    for event_name in ["order.canceled", "order.refunded"] {
        match find_delivery(&deliveries, event_name) {
            Some(delivery) => print_attempt(delivery),
            None => println!("cannot find event {event_name}"),
        };
    }
}

fn print_attempt(delivery: &Delivery) {
    println!(
        "Delivery {}, attempt {} ({})",
        delivery.event, delivery.attempts, delivery.status
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, Bytes, to_bytes},
        http::{Request, StatusCode},
        response::Response,
    };
    use hookrun::db;
    use serde_json::Value;
    use tower::ServiceExt;

    #[sqlx::test]
    async fn blank_event_returns_json_error(pool: sqlx::PgPool) {
        let request = Request::post("/echo")
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"event":" "}"#));

        let response = app(pool).oneshot(request.unwrap()).await.unwrap();
        let body = assert_json_response(response, StatusCode::BAD_REQUEST).await;
        assert_eq!(body["error"], "event must not be blank");
    }

    #[sqlx::test]
    async fn valid_event_is_echoed(pool: sqlx::PgPool) {
        let request = Request::post("/echo")
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"event":"order.created"}"#))
            .unwrap();

        let response = app(pool).oneshot(request).await.unwrap();
        let body = assert_json_response(response, StatusCode::OK).await;
        assert_eq!(body["event"], "order.created");
    }

    #[sqlx::test]
    async fn missing_event_is_rejected(pool: sqlx::PgPool) {
        let request = Request::post("/echo")
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{}"#))
            .unwrap();

        let response = app(pool).oneshot(request).await.unwrap();
        let text = assert_plain_text_response(response, StatusCode::UNPROCESSABLE_ENTITY).await;
        assert!(text.contains("missing field") && text.contains("event"));
    }

    #[sqlx::test]
    async fn listing_deliveries_preserves_the_list(pool: sqlx::PgPool) {
        let Delivery {
            id,
            event,
            attempts,
            status,
        } = seed_delivery(&pool).await;
        let app = app(pool);

        let new_response = async || {
            let request = Request::get("/deliveries").body(Body::empty()).unwrap();
            app.clone().oneshot(request).await.unwrap()
        };

        for response in [new_response().await, new_response().await] {
            let body = assert_json_response(response, StatusCode::OK).await;
            let deliveries = body.as_array().expect("expected a delivery list");
            assert_eq!(deliveries.len(), 1);
            assert_eq!(deliveries[0]["id"], id);
            assert_eq!(deliveries[0]["event"], event);
            assert_eq!(deliveries[0]["attempts"], attempts);
            assert_eq!(deliveries[0]["status"], status.to_string());
        }
    }

    #[sqlx::test]
    async fn created_delivery_appears_in_list(pool: sqlx::PgPool) {
        let Delivery {
            id,
            event,
            attempts,
            status,
        } = seed_delivery(&pool).await;
        let app = app(pool);

        let seeded = {
            let request = Request::get("/deliveries").body(Body::empty()).unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            let body = assert_json_response(response, StatusCode::OK).await;
            let deliveries = body.as_array().expect("expected a delivery list");
            assert_eq!(deliveries.len(), 1);
            assert_eq!(deliveries[0]["id"], id);
            assert_eq!(deliveries[0]["event"], event);
            assert_eq!(deliveries[0]["attempts"], attempts);
            assert_eq!(deliveries[0]["status"], status.to_string());

            body
        };

        let created = {
            let request = Request::post("/deliveries")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"event":"order.refunded"}"#))
                .unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            let body = assert_json_response(response, StatusCode::CREATED).await;
            let created_id = body["id"].as_u64().expect("expected a numeric ID");
            assert!(created_id > 0);
            assert_ne!(created_id, id);
            assert_eq!(body["event"], "order.refunded");
            assert_eq!(body["attempts"], 0);
            assert_eq!(body["status"], "pending");

            body
        };

        {
            let request = Request::get("/deliveries").body(Body::empty()).unwrap();
            let response = app.clone().oneshot(request).await.unwrap();

            let body = assert_json_response(response, StatusCode::OK).await;
            let deliveries = body.as_array().expect("expected a delivery list");
            assert_eq!(deliveries.len(), 2);
            assert!(deliveries.contains(&seeded[0]));
            assert!(deliveries.contains(&created));
        };
    }

    #[sqlx::test]
    async fn blank_delivery_does_not_change_the_list(pool: sqlx::PgPool) {
        let Delivery {
            id,
            event,
            attempts,
            status,
        } = seed_delivery(&pool).await;
        let app = app(pool);

        let seeded = {
            let request = Request::get("/deliveries").body(Body::empty()).unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            let body = assert_json_response(response, StatusCode::OK).await;
            let deliveries = body.as_array().expect("expected a delivery list");
            assert_eq!(deliveries.len(), 1);
            assert_eq!(deliveries[0]["id"], id);
            assert_eq!(deliveries[0]["event"], event);
            assert_eq!(deliveries[0]["attempts"], attempts);
            assert_eq!(deliveries[0]["status"], status.to_string());

            body
        };

        {
            let request = Request::post("/deliveries")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"event":" "}"#))
                .unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            let body = assert_json_response(response, StatusCode::BAD_REQUEST).await;
            assert_eq!(body["error"], "event must not be blank");
        };

        {
            let request = Request::get("/deliveries").body(Body::empty()).unwrap();
            let response = app.clone().oneshot(request).await.unwrap();

            let body = assert_json_response(response, StatusCode::OK).await;
            assert_eq!(body, seeded);
        };
    }

    #[sqlx::test]
    async fn existing_delivery_is_returned_by_id(pool: sqlx::PgPool) {
        let Delivery {
            id,
            event,
            attempts,
            status,
        } = db::create_delivery(&pool, "invoice.paid")
            .await
            .expect("seed delivery");
        let app = app(pool);

        let request = Request::get(format!("/deliveries/{id}"))
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        let body = assert_json_response(response, StatusCode::OK).await;
        assert_eq!(body["id"], id);
        assert_eq!(body["event"], event);
        assert_eq!(body["attempts"], attempts);
        assert_eq!(body["status"], status.to_string());
    }

    #[sqlx::test]
    async fn unknown_delivery_id_returns_not_found(pool: sqlx::PgPool) {
        let app = app(pool);

        let request = Request::get("/deliveries/999").body(Body::empty()).unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        let body = assert_json_response(response, StatusCode::NOT_FOUND).await;
        assert_eq!(body["error"], "delivery not found");
    }

    #[sqlx::test]
    async fn nonnumeric_delivery_id_returns_bad_request(pool: sqlx::PgPool) {
        let app = app(pool);

        let request = Request::get("/deliveries/nope")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        let text = assert_plain_text_response(response, StatusCode::BAD_REQUEST).await;
        assert!(text.contains("nope"));
    }

    #[sqlx::test]
    async fn concurrent_creates_get_distinct_ids(pool: sqlx::PgPool) {
        let Delivery {
            id,
            event,
            attempts,
            status,
        } = seed_delivery(&pool).await;
        let app = app(pool);

        let seeded = {
            let request = Request::get("/deliveries").body(Body::empty()).unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            let body = assert_json_response(response, StatusCode::OK).await;
            let deliveries = body.as_array().expect("expected a delivery list");
            assert_eq!(deliveries.len(), 1);
            assert_eq!(deliveries[0]["id"], id);
            assert_eq!(deliveries[0]["event"], event);
            assert_eq!(deliveries[0]["attempts"], attempts);
            assert_eq!(deliveries[0]["status"], status.to_string());

            body
        };

        let refunded_router = app.clone();
        let shipped_router = app.clone();

        let refunded_task = tokio::spawn(async move {
            let request = Request::post("/deliveries")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"event":"order.refunded"}"#))
                .unwrap();
            refunded_router.oneshot(request).await.unwrap()
        });

        let shipped_task = tokio::spawn(async move {
            let request = Request::post("/deliveries")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"event":"order.shipped"}"#))
                .unwrap();
            shipped_router.oneshot(request).await.unwrap()
        });

        let refunded_response = refunded_task.await.unwrap();
        let shipped_response = shipped_task.await.unwrap();

        let refunded = assert_json_response(refunded_response, StatusCode::CREATED).await;
        refunded["id"].as_u64().expect("expected an u64 integer");
        assert_eq!(refunded["event"], "order.refunded");
        assert_eq!(refunded["attempts"], 0);
        assert_eq!(refunded["status"], "pending");

        let shipped = assert_json_response(shipped_response, StatusCode::CREATED).await;
        shipped["id"].as_u64().expect("expected an u64 integer");
        assert_eq!(shipped["event"], "order.shipped");
        assert_eq!(shipped["attempts"], 0);
        assert_eq!(shipped["status"], "pending");

        assert_ne!(refunded["id"], shipped["id"]);

        {
            let request = Request::get("/deliveries").body(Body::empty()).unwrap();
            let response = app.clone().oneshot(request).await.unwrap();

            let body = assert_json_response(response, StatusCode::OK).await;
            let deliveries = body.as_array().expect("expected a delivery list");
            assert_eq!(deliveries.len(), 3);
            assert!(deliveries.contains(&seeded[0]));
            assert!(deliveries.contains(&refunded));
            assert!(deliveries.contains(&shipped));
        };
    }

    #[sqlx::test]
    async fn oversized_delivery_id_returns_bad_request(pool: sqlx::PgPool) {
        let app = app(pool);

        let request = Request::get(format!("/deliveries/{}", u64::MAX))
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        let body = assert_json_response(response, StatusCode::BAD_REQUEST).await;
        assert_eq!(body["error"], "delivery ID exceeds the supported range")
    }

    #[sqlx::test]
    async fn database_failure_returns_internal_server_error(pool: sqlx::PgPool) {
        pool.close().await;
        let app = app(pool);

        let request = Request::get("/deliveries").body(Body::empty()).unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        let body = assert_json_response(response, StatusCode::INTERNAL_SERVER_ERROR).await;
        assert_eq!(body["error"], "internal server error")
    }

    #[sqlx::test]
    async fn same_idempotency_key_replays_delivery(pool: sqlx::PgPool) {
        let key = "order-created-123";
        let app = app(pool);

        let created = {
            let request = Request::post("/deliveries")
                .header("Content-Type", "application/json")
                .header("Idempotency-Key", key)
                .body(Body::from(r#"{"event":"order.created"}"#))
                .unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            assert_json_response(response, StatusCode::CREATED).await
        };

        let replayed = {
            let request = Request::post("/deliveries")
                .header("Content-Type", "application/json")
                .header("Idempotency-Key", key)
                .body(Body::from(r#"{"event":"order.created"}"#))
                .unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            assert_json_response(response, StatusCode::CREATED).await
        };

        assert_eq!(created, replayed);

        let request = Request::get("/deliveries").body(Body::empty()).unwrap();
        let response = app.clone().oneshot(request).await.unwrap();

        let body = assert_json_response(response, StatusCode::OK).await;
        let deliveries = body.as_array().expect("expected a delivery list");
        assert_eq!(deliveries.len(), 1);
        assert_eq!(deliveries[0], created);
    }

    #[sqlx::test]
    async fn same_idempotency_key_with_different_event_returns_conflict(pool: sqlx::PgPool) {
        let key = "order-created-123";
        let app = app(pool);

        let created = {
            let request = Request::post("/deliveries")
                .header("Content-Type", "application/json")
                .header("Idempotency-Key", key)
                .body(Body::from(r#"{"event":"order.created"}"#))
                .unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            assert_json_response(response, StatusCode::CREATED).await
        };

        {
            let request = Request::post("/deliveries")
                .header("Content-Type", "application/json")
                .header("Idempotency-Key", key)
                .body(Body::from(r#"{"event":"order.shipped"}"#))
                .unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            let body = assert_json_response(response, StatusCode::CONFLICT).await;
            assert_eq!(
                body["error"],
                "idempotency key was already used with a different event"
            )
        };

        let request = Request::get("/deliveries").body(Body::empty()).unwrap();
        let response = app.clone().oneshot(request).await.unwrap();

        let body = assert_json_response(response, StatusCode::OK).await;
        let deliveries = body.as_array().expect("expected a delivery list");
        assert_eq!(deliveries.len(), 1);
        assert_eq!(deliveries[0], created);
    }

    #[sqlx::test]
    async fn blank_idempotency_key_returns_bad_request(pool: sqlx::PgPool) {
        let key = " ";
        let app = app(pool);

        {
            let request = Request::post("/deliveries")
                .header("Content-Type", "application/json")
                .header("Idempotency-Key", key)
                .body(Body::from(r#"{"event":"order.created"}"#))
                .unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            let body = assert_json_response(response, StatusCode::BAD_REQUEST).await;
            assert_eq!(body["error"], "idempotency key must not be blank");
        };

        let request = Request::get("/deliveries").body(Body::empty()).unwrap();
        let response = app.clone().oneshot(request).await.unwrap();

        let body = assert_json_response(response, StatusCode::OK).await;
        let deliveries = body.as_array().expect("expected a delivery list");
        assert_eq!(deliveries.len(), 0);
    }

    #[sqlx::test]
    async fn invalid_idempotency_key_header_returns_bad_request(pool: sqlx::PgPool) {
        let key = axum::http::HeaderValue::from_bytes(&[0xFF]).unwrap();
        let app = app(pool);

        {
            let request = Request::post("/deliveries")
                .header("Content-Type", "application/json")
                .header("Idempotency-Key", key)
                .body(Body::from(r#"{"event":"order.created"}"#))
                .unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            let body = assert_json_response(response, StatusCode::BAD_REQUEST).await;
            assert_eq!(body["error"], "invalid Idempotency-Key header");
        };

        let request = Request::get("/deliveries").body(Body::empty()).unwrap();
        let response = app.clone().oneshot(request).await.unwrap();

        let body = assert_json_response(response, StatusCode::OK).await;
        let deliveries = body.as_array().expect("expected a delivery list");
        assert_eq!(deliveries.len(), 0);
    }

    async fn assert_json_response(response: Response, expected_status: StatusCode) -> Value {
        let bytes = assert_response(response, expected_status, "application/json").await;
        serde_json::from_slice(&bytes).unwrap()
    }

    async fn assert_plain_text_response(response: Response, expected_status: StatusCode) -> String {
        let bytes = assert_response(response, expected_status, "text/plain; charset=utf-8").await;
        std::str::from_utf8(&bytes).unwrap().to_owned()
    }

    async fn assert_response(
        response: Response,
        expected_status: StatusCode,
        content_type: &'static str,
    ) -> Bytes {
        assert_eq!(response.status(), expected_status);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            content_type
        );

        to_bytes(response.into_body(), 1024).await.unwrap()
    }

    async fn seed_delivery(pool: &sqlx::PgPool) -> Delivery {
        db::create_delivery(pool, "order.created")
            .await
            .expect("seed delivery")
    }
}
