use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::engine::ClobEngine;

static ENGINE: OnceLock<Mutex<ClobEngine>> = OnceLock::new();

pub fn get_engine() -> &'static Mutex<ClobEngine> {
    ENGINE.get_or_init(|| Mutex::new(ClobEngine::new()))
}

pub fn init_engine(engine: ClobEngine) {
    let _ = ENGINE.set(Mutex::new(engine));
}

pub fn lock_engine() -> MutexGuard<'static, ClobEngine> {
    get_engine().lock().unwrap_or_else(|e| e.into_inner())
}
