use axum::{body::Body, http::Request};
use noko_rs::actor::model::ActorKind;
use noko_rs::application::provision_registered_customer::ProvisionRegisteredCustomerResult;
use noko_rs::customer::model::Customer;
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

async fn setup_app(pool: PgPool) -> (axum::Router, noko_rs::app::AppState, Uuid, String) {
    let service_actor_id = Uuid::new_v4();
    let service_token = "secret_service_token".to_string();

    sqlx::query(
        r#"
        INSERT INTO actors (id, kind, auth_subject, display_name, active)
        VALUES ($1, 'service', 'service-test', 'Service Test Actor', true)
        "#,
    )
    .bind(service_actor_id)
    .execute(&pool)
    .await
    .unwrap();

    let config = noko_rs::config::Config {
        database_url: "".to_string(),
        bind_addr: "0.0.0.0:3000".to_string(),
        auth_mode: "dev_header".to_string(),
        nocodb_service_token: Some(service_token.clone()),
        nocodb_service_actor_id: Some(service_actor_id),
        db_tx_max_retries: 2,
    };

    let state = noko_rs::app::AppState {
        pool,
        config: Arc::new(config),
    };
    let app = noko_rs::app::build_router(state.clone());

    (app, state, service_actor_id, service_token)
}

#[sqlx::test(migrations = "./migrations")]
async fn test_provision_customer_happy_path(pool: PgPool) {
    let (app, _state, _, token) = setup_app(pool.clone()).await;

    let payload = json!({
        "auth_subject": "sub-123",
        "display_name": "Test User",
        "email": "test@example.com",
        "phone": "+1234567890",
        "first_name": "Test",
        "last_name": "User",
        "kind": "service",
        "actor_id": Uuid::new_v4(),
        "active": false
    });

    let req = Request::builder()
        .method("POST")
        .uri("/ops/customers")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&payload).unwrap()))
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), 200);

    let body_bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let result: ProvisionRegisteredCustomerResult = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(result.actor.kind, ActorKind::Human);
    assert!(result.actor.active);
    assert_eq!(result.customer.actor_id, result.actor.id);

    let actor_count: i64 = sqlx::query_scalar("SELECT count(*) FROM actors WHERE id = $1")
        .bind(result.actor.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(actor_count, 1);

    let customer_count: i64 = sqlx::query_scalar("SELECT count(*) FROM customers WHERE id = $1")
        .bind(result.customer.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(customer_count, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_provision_customer_unauthorized(pool: PgPool) {
    let (app, _, _, _) = setup_app(pool).await;

    let payload = json!({
        "auth_subject": "sub-123",
        "display_name": "Test User",
        "email": "test@example.com",
        "first_name": "Test",
        "last_name": "User"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/ops/customers")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&payload).unwrap()))
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), 401);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_store_auth_integration(pool: PgPool) {
    let (app, _, _, token) = setup_app(pool.clone()).await;

    let payload = json!({
        "auth_subject": "new-auth-subject",
        "display_name": "Dev User",
        "email": "dev@example.com",
        "first_name": "Dev",
        "last_name": "User"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/ops/customers")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&payload).unwrap()))
        .unwrap();

    let response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), 200);

    let req2 = Request::builder()
        .method("GET")
        .uri("/store/me")
        .header("X-Dev-Auth-Subject", "new-auth-subject")
        .body(Body::empty())
        .unwrap();

    let response2 = app.oneshot(req2).await.unwrap();
    assert_eq!(response2.status(), 200);

    let body_bytes = axum::body::to_bytes(response2.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let customer: Customer = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(customer.email, "dev@example.com");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_provision_customer_rollback(pool: PgPool) {
    let (app, _, _, token) = setup_app(pool.clone()).await;

    let payload1 = json!({
        "auth_subject": "sub-duplicate-1",
        "display_name": "First User",
        "email": "conflict@example.com",
        "first_name": "First",
        "last_name": "User"
    });

    let req1 = Request::builder()
        .method("POST")
        .uri("/ops/customers")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&payload1).unwrap()))
        .unwrap();

    let response1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(response1.status(), 200);

    let payload2 = json!({
        "auth_subject": "sub-duplicate-2",
        "display_name": "Second User",
        "email": "conflict@example.com",
        "first_name": "Second",
        "last_name": "User"
    });

    let req2 = Request::builder()
        .method("POST")
        .uri("/ops/customers")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&payload2).unwrap()))
        .unwrap();

    let response2 = app.oneshot(req2).await.unwrap();
    assert_eq!(response2.status(), 500);

    let actor_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM actors WHERE auth_subject = $1")
            .bind("sub-duplicate-2")
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(actor_count, 0);
}
