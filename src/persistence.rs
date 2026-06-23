use rust_decimal::Decimal;

use crate::engine::ClobEngine;
use crate::error::ClobError;
use crate::types::Order;


// Data types for loading engine state from Postgres (or test fixtures)


/// Instrument definition as read from `clob.instruments`.
#[derive(Clone, Debug, PartialEq)]
pub struct InstrumentDef {
    pub id: u64,
    pub symbol: String,
    pub tick_size: Decimal,
    pub lot_size: Decimal,
    pub max_ticks: usize,
}


// Engine rebuild — start-up recovery path


/// Rebuild the engine state from instrument definitions and open orders.
///
/// Orders **must** be sorted by `created_at ASC` before calling this
/// function so that time priority is preserved across restarts.
///
/// This is the Rust-side equivalent of `load_from_postgres()`.  The pgrx
/// bindings in Phase 7 will read the instrument/order rows via SPI and
/// convert them into `InstrumentDef` / `Order` before handing them off here.
pub fn rebuild_book(
    engine: &mut ClobEngine,
    instruments: &[InstrumentDef],
    orders: &[Order],
) -> Result<(), ClobError> {
    // 1. Create instruments — allocate tick arrays
    for inst in instruments {
        engine.create_instrument_with_id(
            inst.id,
            &inst.symbol,
            inst.tick_size,
            inst.lot_size,
            inst.max_ticks,
        )?;
    }

    // 2. Replay open orders in timestamp order (already sorted by caller).
    //    `load_order` inserts into the book directly without matching.
    for order in orders {
        engine.load_order(order.clone())?;
    }

    Ok(())
}
