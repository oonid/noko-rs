sed -i 's/unsafe { env::set_var("DATABASE_URL", "postgres:\/\/test") };//g' src/config.rs
