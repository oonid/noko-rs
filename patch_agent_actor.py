import sys

with open("tests/operations.rs", "r") as f:
    ops = f.read()

agent_test = """
    let agent_actor_id = Uuid::new_v4();
    sqlx::query("INSERT INTO actors (id, kind, auth_subject, active, display_name) VALUES ($1, 'agent', $2, true, 'agent')").bind(agent_actor_id).bind(format!("sub_{}", agent_actor_id)).execute(&pool).await.unwrap();
    let app_agent = setup_app(pool.clone(), agent_actor_id, "test_token").await;
    let req = Request::builder()
        .method("POST")
        .uri("/ops/catalog/variants")
        .header("authorization", "Bearer test_token")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let res = app_agent.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["code"], "ACTOR_NOT_SERVICE");
}
"""

ops = ops.replace('    assert_eq!(body["code"], "ACTOR_NOT_SERVICE");\n}', '    assert_eq!(body["code"], "ACTOR_NOT_SERVICE");\n' + agent_test)

with open("tests/operations.rs", "w") as f:
    f.write(ops)
