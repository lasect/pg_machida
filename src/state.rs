use std::sync::{Mutex, OnceLock};

use crate::engine::ClobEngine;

static ENGINE: OnceLock<Mutex<ClobEngine>> = OnceLock::new();

pub fn get_engine() -> &'static Mutex<ClobEngine> {
    ENGINE.get_or_init(|| Mutex::new(ClobEngine::new()))
}
