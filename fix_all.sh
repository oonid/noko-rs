sed -i 's/pub struct Config {/#[derive(Debug)]\npub struct Config {/g' src/config.rs
sed -i 's/lazy_static::lazy_static! {/static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());\n\/*/' src/config.rs
sed -i 's/static ref ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());/*/' src/config.rs
sed -i 's/    }/*\//' src/config.rs
sed -i 's/env::remove_var/unsafe { env::remove_var("AUTH_MODE") }/g' src/config.rs
sed -i 's/env::set_var(/unsafe { env::set_var(/g' src/config.rs
sed -i 's/dev");/dev"); }/g' src/config.rs
sed -i 's/unsupported");/unsupported"); }/g' src/config.rs
sed -i 's/dev_header");/dev_header"); }/g' src/config.rs
sed -i 's/"postgres:\/\/test");/"postgres:\/\/test"); }/g' src/config.rs

sed -i 's/sqlx::query!("SELECT customer_id FROM customer_addresses WHERE id = $1", Uuid::parse_str(json\["id"\].as_str().unwrap()).unwrap())/sqlx::query_scalar::<_, Uuid>("SELECT customer_id FROM customer_addresses WHERE id = $1").bind(Uuid::parse_str(json["id"].as_str().unwrap()).unwrap())/g' tests/customer_auth.rs
