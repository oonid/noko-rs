use axum::{Router, routing::get};
#[test]
fn axum_test() {
    let _app = Router::<()>::new().route("/users/{id}", get(|| async {}));
}
