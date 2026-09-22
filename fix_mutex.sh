sed -i 's/let guard = ENV_MUTEX.lock().unwrap();/let guard = match ENV_MUTEX.lock() { Ok(g) => g, Err(p) => p.into_inner() };/g' src/config.rs
