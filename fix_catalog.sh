cat << 'INNER_EOF' >> tests/catalog.rs

#[sqlx::test(migrations = "./migrations")]
async fn test_public_catalog_no_auth(pool: sqlx::PgPool) {
    let app = setup_test_app(pool.clone()).await;

    let req = axum::http::Request::builder()
        .uri("/store/products")
        .body(axum::body::Body::empty())
        .unwrap();

    let res = tower::ServiceExt::oneshot(app, req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::OK);
}
INNER_EOF
