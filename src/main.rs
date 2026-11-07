mod api_error;
mod deliveries;
mod delivery;
mod echo;
mod health;

use std::sync::{Arc, Mutex};

use axum::{
    Router,
    routing::{get, post},
};
use delivery::{Delivery, DeliveryStatus, find_delivery, mark_failed, mark_succeeded};

struct DeliveryStore {
    deliveries: Vec<Delivery>,
    next_id: u64,
}

#[tokio::main]
async fn main() {
    run_demo();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    axum::serve(listener, app()).await.unwrap();
}

fn app() -> Router {
    let deliveries = vec![Delivery {
        id: 1,
        event: String::from("order.created"),
        attempts: 0,
        status: DeliveryStatus::Pending,
    }];
    let delivery_store = Arc::new(Mutex::new(DeliveryStore {
        deliveries,
        next_id: 2,
    }));

    Router::new()
        .route("/health", get(health::health))
        .route("/echo", post(echo::echo))
        .route(
            "/deliveries",
            get(deliveries::list_deliveries).post(deliveries::create_delivery),
        )
        .route("/deliveries/{id}", get(deliveries::get_delivery))
        .with_state(delivery_store)
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
    use serde_json::Value;
    use tower::ServiceExt;

    #[tokio::test]
    async fn blank_event_returns_json_error() {
        let request = Request::post("/echo")
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"event":" "}"#));

        let response = app().oneshot(request.unwrap()).await.unwrap();
        let body = assert_json_response(response, StatusCode::BAD_REQUEST).await;
        assert_eq!(body["error"], "event must not be blank");
    }

    #[tokio::test]
    async fn valid_event_is_echoed() {
        let request = Request::post("/echo")
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"event":"order.created"}"#))
            .unwrap();

        let response = app().oneshot(request).await.unwrap();
        let body = assert_json_response(response, StatusCode::OK).await;
        assert_eq!(body["event"], "order.created");
    }

    #[tokio::test]
    async fn missing_event_is_rejected() {
        let request = Request::post("/echo")
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{}"#))
            .unwrap();

        let response = app().oneshot(request).await.unwrap();
        let text = assert_plain_text_response(response, StatusCode::UNPROCESSABLE_ENTITY).await;
        assert!(text.contains("missing field") && text.contains("event"));
    }

    #[tokio::test]
    async fn listing_deliveries_preserves_the_list() {
        let app = app();

        let new_response = async || {
            let request = Request::get("/deliveries").body(Body::empty()).unwrap();
            app.clone().oneshot(request).await.unwrap()
        };

        for response in [new_response().await, new_response().await] {
            let body = assert_json_response(response, StatusCode::OK).await;
            let deliveries = body.as_array().expect("expected a delivery list");
            assert_eq!(deliveries.len(), 1);
            assert_eq!(deliveries[0]["id"], 1);
            assert_eq!(deliveries[0]["event"], "order.created");
            assert_eq!(deliveries[0]["attempts"], 0);
            assert_eq!(deliveries[0]["status"], "pending");
        }
    }

    #[tokio::test]
    async fn created_delivery_appears_in_list() {
        let app = app();

        let seeded = {
            let request = Request::get("/deliveries").body(Body::empty()).unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            let body = assert_json_response(response, StatusCode::OK).await;
            let deliveries = body.as_array().expect("expected a delivery list");
            assert_eq!(deliveries.len(), 1);
            assert_eq!(deliveries[0]["id"], 1);
            assert_eq!(deliveries[0]["event"], "order.created");
            assert_eq!(deliveries[0]["attempts"], 0);
            assert_eq!(deliveries[0]["status"], "pending");

            body
        };

        let created = {
            let request = Request::post("/deliveries")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"event":"order.refunded"}"#))
                .unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            let body = assert_json_response(response, StatusCode::CREATED).await;
            assert_eq!(body["id"], 2);
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

    #[tokio::test]
    async fn blank_delivery_does_not_change_the_list() {
        let app = app();

        let seeded = {
            let request = Request::get("/deliveries").body(Body::empty()).unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            let body = assert_json_response(response, StatusCode::OK).await;
            let deliveries = body.as_array().expect("expected a delivery list");
            assert_eq!(deliveries.len(), 1);
            assert_eq!(deliveries[0]["id"], 1);
            assert_eq!(deliveries[0]["event"], "order.created");
            assert_eq!(deliveries[0]["attempts"], 0);
            assert_eq!(deliveries[0]["status"], "pending");

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

    #[tokio::test]
    async fn existing_delivery_is_returned_by_id() {
        let app = app();

        let request = Request::get("/deliveries/1").body(Body::empty()).unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        let body = assert_json_response(response, StatusCode::OK).await;
        assert_eq!(body["id"], 1);
        assert_eq!(body["event"], "order.created");
        assert_eq!(body["attempts"], 0);
        assert_eq!(body["status"], "pending");
    }

    #[tokio::test]
    async fn unknown_delivery_id_returns_not_found() {
        let app = app();

        let request = Request::get("/deliveries/999").body(Body::empty()).unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        let body = assert_json_response(response, StatusCode::NOT_FOUND).await;
        assert_eq!(body["error"], "delivery not found");
    }

    #[tokio::test]
    async fn nonnumeric_delivery_id_returns_bad_request() {
        let app = app();

        let request = Request::get("/deliveries/nope")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        let text = assert_plain_text_response(response, StatusCode::BAD_REQUEST).await;
        assert!(text.contains("nope"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn concurrent_creates_get_distinct_ids() {
        let app = app();

        let seeded = {
            let request = Request::get("/deliveries").body(Body::empty()).unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            let body = assert_json_response(response, StatusCode::OK).await;
            let deliveries = body.as_array().expect("expected a delivery list");
            assert_eq!(deliveries.len(), 1);
            assert_eq!(deliveries[0]["id"], 1);
            assert_eq!(deliveries[0]["event"], "order.created");
            assert_eq!(deliveries[0]["attempts"], 0);
            assert_eq!(deliveries[0]["status"], "pending");

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
}
