use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use pgrx::prelude::*;
use pgrx::spi::Spi;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::engine::ClobEngine;
use crate::persistence::{rebuild_book, InstrumentDef};
use crate::types::*;

::pgrx::pg_module_magic!(name, version);
::pgrx::extension_sql_file!("../sql/pg_machida--0.1.0.sql", name = "schema", bootstrap);

pub mod types;
pub mod book;
pub mod engine;
pub mod matching;
pub mod state;
pub mod persistence;
pub mod background_worker;
pub mod notify;
pub mod error;

fn now_nanos() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

fn f64_to_decimal(val: f64) -> Decimal {
    Decimal::from_f64(val).unwrap_or_else(|| pgrx::error!("invalid numeric value: {}", val))
}

fn text_sql(text: &str) -> String {
    let escaped = text.replace('\'', "''");
    format!("'{}'", escaped)
}

fn nullable_decimal_sql(value: Option<Decimal>) -> String {
    value
        .map(|d| format!("{}::numeric", d))
        .unwrap_or_else(|| "NULL".to_string())
}

fn parse_decimal(value: &str, column: &str) -> Decimal {
    Decimal::from_str(value).unwrap_or_else(|e| pgrx::error!("invalid {column}: {e}"))
}

// ---------------------------------------------------------------------------
// load_from_postgres — rebuild in-memory engine from persisted rows
// ---------------------------------------------------------------------------

#[pg_extern]
fn clob_load_from_postgres() -> i64 {
    let (instruments, orders) = Spi::connect(|client| {
        let instruments_table = client
            .select(
                "SELECT id, symbol, tick_size::text, lot_size::text, max_ticks \
                 FROM clob.instruments \
                 WHERE status = 'active' \
                 ORDER BY id",
                None,
                &[],
            )
            .unwrap_or_else(|e| pgrx::error!("failed to load instruments: {e}"));

        let mut instruments = Vec::new();
        for row in instruments_table {
            let id = row
                .get_by_name::<i64, _>("id")
                .unwrap_or_else(|e| pgrx::error!("failed to read instrument id: {e}"))
                .unwrap_or_else(|| pgrx::error!("instrument id is null"));
            let symbol = row
                .get_by_name::<String, _>("symbol")
                .unwrap_or_else(|e| pgrx::error!("failed to read instrument symbol: {e}"))
                .unwrap_or_else(|| pgrx::error!("instrument symbol is null"));
            let tick_size = row
                .get_by_name::<String, _>("tick_size")
                .unwrap_or_else(|e| pgrx::error!("failed to read tick_size: {e}"))
                .unwrap_or_else(|| pgrx::error!("tick_size is null"));
            let lot_size = row
                .get_by_name::<String, _>("lot_size")
                .unwrap_or_else(|e| pgrx::error!("failed to read lot_size: {e}"))
                .unwrap_or_else(|| pgrx::error!("lot_size is null"));
            let max_ticks = row
                .get_by_name::<i32, _>("max_ticks")
                .unwrap_or_else(|e| pgrx::error!("failed to read max_ticks: {e}"))
                .unwrap_or_else(|| pgrx::error!("max_ticks is null"));

            instruments.push(InstrumentDef {
                id: id as u64,
                symbol,
                tick_size: parse_decimal(&tick_size, "tick_size"),
                lot_size: parse_decimal(&lot_size, "lot_size"),
                max_ticks: max_ticks as usize,
            });
        }

        let orders_table = client
            .select(
                "SELECT id::text, instrument_id, participant_id, side, order_type, \
                        price::text, qty::text, remaining::text, status, stp_mode, \
                        (extract(epoch from created_at) * 1000000000)::bigint AS ts \
                 FROM clob.orders \
                 WHERE status IN ('open', 'partially_filled') \
                 ORDER BY created_at ASC, id ASC",
                None,
                &[],
            )
            .unwrap_or_else(|e| pgrx::error!("failed to load orders: {e}"));

        let mut orders = Vec::new();
        for row in orders_table {
            let id = row
                .get_by_name::<String, _>("id")
                .unwrap_or_else(|e| pgrx::error!("failed to read order id: {e}"))
                .unwrap_or_else(|| pgrx::error!("order id is null"));
            let instrument_id = row
                .get_by_name::<i64, _>("instrument_id")
                .unwrap_or_else(|e| pgrx::error!("failed to read order instrument_id: {e}"))
                .unwrap_or_else(|| pgrx::error!("order instrument_id is null"));
            let participant_id = row
                .get_by_name::<String, _>("participant_id")
                .unwrap_or_else(|e| pgrx::error!("failed to read participant_id: {e}"))
                .unwrap_or_else(|| pgrx::error!("participant_id is null"));
            let side = row
                .get_by_name::<String, _>("side")
                .unwrap_or_else(|e| pgrx::error!("failed to read side: {e}"))
                .unwrap_or_else(|| pgrx::error!("side is null"));
            let order_type = row
                .get_by_name::<String, _>("order_type")
                .unwrap_or_else(|e| pgrx::error!("failed to read order_type: {e}"))
                .unwrap_or_else(|| pgrx::error!("order_type is null"));
            let price = row
                .get_by_name::<String, _>("price")
                .unwrap_or_else(|e| pgrx::error!("failed to read price: {e}"));
            let qty = row
                .get_by_name::<String, _>("qty")
                .unwrap_or_else(|e| pgrx::error!("failed to read qty: {e}"))
                .unwrap_or_else(|| pgrx::error!("qty is null"));
            let remaining = row
                .get_by_name::<String, _>("remaining")
                .unwrap_or_else(|e| pgrx::error!("failed to read remaining: {e}"))
                .unwrap_or_else(|| pgrx::error!("remaining is null"));
            let status = row
                .get_by_name::<String, _>("status")
                .unwrap_or_else(|e| pgrx::error!("failed to read status: {e}"))
                .unwrap_or_else(|| pgrx::error!("status is null"));
            let stp_mode = row
                .get_by_name::<String, _>("stp_mode")
                .unwrap_or_else(|e| pgrx::error!("failed to read stp_mode: {e}"))
                .unwrap_or_else(|| pgrx::error!("stp_mode is null"));
            let ts = row
                .get_by_name::<i64, _>("ts")
                .unwrap_or_else(|e| pgrx::error!("failed to read order timestamp: {e}"))
                .unwrap_or_else(|| pgrx::error!("order timestamp is null"));

            let mut order = Order::new(
                Uuid::parse_str(&id).unwrap_or_else(|e| pgrx::error!("invalid order id: {e}")),
                instrument_id as u64,
                participant_id,
                side.as_str()
                    .try_into()
                    .unwrap_or_else(|e: String| pgrx::error!("{e}")),
                order_type
                    .as_str()
                    .try_into()
                    .unwrap_or_else(|e: String| pgrx::error!("{e}")),
                price.as_deref().map(|p| parse_decimal(p, "price")),
                parse_decimal(&qty, "qty"),
                ts as u64,
                stp_mode
                    .as_str()
                    .try_into()
                    .unwrap_or_else(|e: String| pgrx::error!("{e}")),
            );
            order.remaining = parse_decimal(&remaining, "remaining");
            order.status = status
                .as_str()
                .try_into()
                .unwrap_or_else(|e: String| pgrx::error!("{e}"));
            orders.push(order);
        }

        (instruments, orders)
    });

    let mut rebuilt = ClobEngine::new();
    rebuild_book(&mut rebuilt, &instruments, &orders)
        .unwrap_or_else(|e| pgrx::error!("failed to rebuild engine: {e}"));

    let mut engine = state::lock_engine();
    *engine = rebuilt;

    orders.len() as i64
}

fn persist_order(
    order_id: Uuid,
    instrument_id: u64,
    participant: &str,
    side: Side,
    order_type: OrderType,
    price: Option<Decimal>,
    qty: Decimal,
    remaining: Decimal,
    status: OrderStatus,
    stp_mode: STPMode,
) {
    let side_str: &str = side.into();
    let order_type_str: &str = order_type.into();
    let status_str: &str = status.into();
    let stp_mode_str: &str = stp_mode.into();

    Spi::run(&format!(
        "INSERT INTO clob.orders \
         (id, instrument_id, participant_id, side, order_type, price, qty, remaining, status, stp_mode) \
         VALUES ({}::uuid, {}, {}, {}, {}, {}, {}::numeric, {}::numeric, {}, {}) \
         ON CONFLICT (id) DO UPDATE SET \
         remaining = EXCLUDED.remaining, status = EXCLUDED.status, updated_at = now()",
        text_sql(&order_id.to_string()),
        instrument_id,
        text_sql(participant),
        text_sql(side_str),
        text_sql(order_type_str),
        nullable_decimal_sql(price),
        qty,
        remaining,
        text_sql(status_str),
        text_sql(stp_mode_str),
    ))
    .unwrap_or_else(|e| pgrx::error!("failed to persist order: {e}"));
}

fn decrement_persisted_order(order_id: Uuid, fill_qty: Decimal) {
    Spi::run(&format!(
        "UPDATE clob.orders \
         SET remaining = GREATEST(remaining - {}::numeric, 0::numeric), \
             status = CASE \
                 WHEN GREATEST(remaining - {}::numeric, 0::numeric) = 0::numeric THEN 'filled' \
                 ELSE 'partially_filled' \
             END, \
             updated_at = now() \
         WHERE id = {}::uuid",
        fill_qty,
        fill_qty,
        text_sql(&order_id.to_string()),
    ))
    .unwrap_or_else(|e| pgrx::error!("failed to update filled order: {e}"));
}

// ---------------------------------------------------------------------------
// create_instrument
// ---------------------------------------------------------------------------

#[pg_extern]
fn clob_create_instrument(
    symbol: &str,
    tick_size: f64,
    lot_size: f64,
    max_ticks: i32,
) -> i64 {
    let tick_size_dec = f64_to_decimal(tick_size);
    let lot_size_dec = f64_to_decimal(lot_size);

    let id: i64 = Spi::get_one::<i64>(
        &format!(
            "INSERT INTO clob.instruments (symbol, tick_size, lot_size, max_ticks) \
             VALUES ({}, {}::numeric, {}::numeric, {}) RETURNING id",
            text_sql(symbol),
            tick_size_dec,
            lot_size_dec,
            max_ticks
        )
    )
    .unwrap_or_else(|e| pgrx::error!("{e}"))
    .unwrap_or_else(|| pgrx::error!("failed to insert instrument"));

    let result = {
        let mut engine = crate::state::lock_engine();
        engine.create_instrument_with_id(
            id as u64,
            symbol,
            tick_size_dec,
            lot_size_dec,
            max_ticks as usize,
        )
    };
    result.unwrap_or_else(|e| pgrx::error!("{e}"));

    id
}

// ---------------------------------------------------------------------------
// place_order
// ---------------------------------------------------------------------------

#[pg_extern]
fn clob_place_order(
    instrument: &str,
    side: &str,
    order_type: &str,
    qty: f64,
    participant: &str,
    price: Option<f64>,
    stp_mode: Option<&str>,
) -> TableIterator<
    'static,
    (
        name!(order_id, String),
        name!(status, String),
        name!(filled_qty, f64),
        name!(avg_price, Option<f64>),
    ),
> {
    let side: Side = side
        .try_into()
        .unwrap_or_else(|e: String| pgrx::error!("{e}"));
    let order_type: OrderType = order_type
        .try_into()
        .unwrap_or_else(|e: String| pgrx::error!("{e}"));
    let stp_mode: STPMode = stp_mode
        .unwrap_or("cancel_newest")
        .try_into()
        .unwrap_or_else(|e: String| pgrx::error!("{e}"));

    let price_dec = price.map(f64_to_decimal);
    let qty_dec = f64_to_decimal(qty);

    let (instr_id_opt, engine_result) = {
        let mut engine = crate::state::lock_engine();
        let id = engine.instrument_id(instrument);
        let result = id.map(|i| {
            let order = Order::new(
                Uuid::new_v4(),
                i,
                participant.to_string(),
                side,
                order_type,
                price_dec,
                qty_dec,
                now_nanos(),
                stp_mode,
            );
            engine.place_order(order)
        });
        (id, result)
    };

    let instr_id = instr_id_opt
        .unwrap_or_else(|| pgrx::error!("instrument not found: {}", instrument));
    let result = engine_result.expect("instrument_id was Some so engine_result must be Some");
    let result = result.unwrap_or_else(|e| pgrx::error!("{e}"));

    let order_id_str = result.order_id.to_string();
    let status_str: &str = result.status.into();
    let persisted_remaining = if matches!(order_type, OrderType::Limit)
        && matches!(result.status, OrderStatus::Open | OrderStatus::PartiallyFilled)
    {
        qty_dec - result.filled_qty
    } else {
        Decimal::ZERO
    };

    persist_order(
        result.order_id,
        instr_id,
        participant,
        side,
        order_type,
        price_dec,
        qty_dec,
        persisted_remaining,
        result.status,
        stp_mode,
    );

    for trade in &result.trades {
        if trade.buy_order_id != result.order_id {
            decrement_persisted_order(trade.buy_order_id, trade.qty);
        }
        if trade.sell_order_id != result.order_id {
            decrement_persisted_order(trade.sell_order_id, trade.qty);
        }
    }

    let filled_qty_f64 = result.filled_qty
        .to_f64()
        .unwrap_or_else(|| pgrx::error!("failed to convert filled_qty to f64"));
    let avg_price_f64 = result.avg_fill_price.and_then(|d| d.to_f64());

    for trade in &result.trades {
        Spi::run(&format!(
            "INSERT INTO clob.trades (id, instrument_id, buy_order_id, sell_order_id, \
             buy_participant_id, sell_participant_id, price, qty) \
             VALUES ({}::uuid, {}, {}::uuid, {}::uuid, {}, {}, {}::numeric, {}::numeric) \
             ON CONFLICT (id) DO NOTHING",
            text_sql(&trade.id.to_string()),
            trade.instrument_id,
            text_sql(&trade.buy_order_id.to_string()),
            text_sql(&trade.sell_order_id.to_string()),
            text_sql(&trade.buy_participant_id),
            text_sql(&trade.sell_participant_id),
            trade.price,
            trade.qty,
        ))
        .unwrap_or_else(|e| pgrx::error!("failed to insert trade: {e}"));
    }

    TableIterator::new(vec![(
        order_id_str,
        status_str.to_string(),
        filled_qty_f64,
        avg_price_f64,
    )])
}

// ---------------------------------------------------------------------------
// cancel_order
// ---------------------------------------------------------------------------

#[pg_extern]
fn clob_cancel_order(order_id: &str) -> bool {
    let id =
        Uuid::parse_str(order_id).unwrap_or_else(|e| pgrx::error!("invalid UUID: {e}"));

    let result = {
        let mut engine = crate::state::lock_engine();
        engine.cancel_order(id)
    };

    match result {
        Ok(cancelled) => {
            Spi::run(&format!(
                "UPDATE clob.orders \
                 SET remaining = 0::numeric, status = 'cancelled', updated_at = now() \
                 WHERE id = {}::uuid",
                text_sql(&cancelled.id.to_string()),
            ))
            .unwrap_or_else(|e| pgrx::error!("failed to persist cancellation: {e}"));
            true
        }
        Err(e) => {
            pgrx::warning!("cancel failed: {e}");
            false
        }
    }
}

// ---------------------------------------------------------------------------
// get_book — depth snapshot from engine
// ---------------------------------------------------------------------------

#[pg_extern]
fn clob_get_book(
    instrument: &str,
    depth: i32,
) -> TableIterator<
    'static,
    (
        name!(side, String),
        name!(price, f64),
        name!(qty, f64),
        name!(order_count, i32),
    ),
> {
    let (instr_id_opt, book_depth_result) = {
        let engine = state::lock_engine();
        let id = engine.instrument_id(instrument);
        let depth = id.map(|i| engine.get_book_depth(i, depth as usize));
        (id, depth)
    };

    let _instr_id = instr_id_opt.unwrap_or_else(|| pgrx::error!("instrument not found: {}", instrument));
    let book_depth = book_depth_result.expect("instrument_id was Some so book_depth must be Some");
    let book_depth = book_depth.unwrap_or_else(|e| pgrx::error!("{e}"));

    let mut rows = Vec::new();

    for bid in &book_depth.bids {
        rows.push((
            "buy".to_string(),
            bid.price.to_f64().unwrap_or(0.0),
            bid.qty.to_f64().unwrap_or(0.0),
            bid.order_count as i32,
        ));
    }

    for ask in &book_depth.asks {
        rows.push((
            "sell".to_string(),
            ask.price.to_f64().unwrap_or(0.0),
            ask.qty.to_f64().unwrap_or(0.0),
            ask.order_count as i32,
        ));
    }

    TableIterator::new(rows)
}

// ---------------------------------------------------------------------------
// get_open_orders — engine state
// ---------------------------------------------------------------------------

#[pg_extern]
fn clob_get_open_orders(
    participant: &str,
    instrument: Option<&str>,
) -> TableIterator<
    'static,
    (
        name!(order_id, String),
        name!(instrument_id, i64),
        name!(side, String),
        name!(order_type, String),
        name!(price, Option<f64>),
        name!(qty, f64),
        name!(remaining, f64),
        name!(status, String),
    ),
> {
    let mut rows: Vec<(
        String,
        i64,
        String,
        String,
        Option<f64>,
        f64,
        f64,
        String,
    )> = Vec::new();

    {
        let engine = state::lock_engine();

        let instr_id = instrument.and_then(|sym| engine.instrument_id(sym));

        match instr_id {
            Some(id) => {
                let orders = engine.get_open_orders(id, participant);
                drop(engine);
                let orders = orders.unwrap_or_else(|e| pgrx::error!("{e}"));
                for o in &orders {
                    rows.push(order_to_row(o, id));
                }
            }
            None => {
                let all_ids: Vec<u64> = engine.all_instrument_ids();
                for id in all_ids {
                    if let Ok(orders) = engine.get_open_orders(id, participant) {
                        for o in &orders {
                            rows.push(order_to_row(o, id));
                        }
                    }
                }
            }
        }
    }

    TableIterator::new(rows)
}

fn order_to_row(
    o: &Order,
    instr_id: u64,
) -> (
    String,
    i64,
    String,
    String,
    Option<f64>,
    f64,
    f64,
    String,
) {
    let side: &str = o.side.into();
    let ot: &str = o.order_type.into();
    let status: &str = o.status.into();
    (
        o.id.to_string(),
        instr_id as i64,
        side.to_string(),
        ot.to_string(),
        o.price.and_then(|d| d.to_f64()),
        o.qty.to_f64().unwrap_or(0.0),
        o.remaining.to_f64().unwrap_or(0.0),
        status.to_string(),
    )
}

// ---------------------------------------------------------------------------
// mass_cancel
// ---------------------------------------------------------------------------

#[pg_extern]
fn clob_mass_cancel(participant: &str, instrument: &str) -> i32 {
    let (instr_id_opt, count_result) = {
        let mut engine = crate::state::lock_engine();
        let id = engine.instrument_id(instrument);
        let result = id.map(|i| engine.mass_cancel(i, participant));
        (id, result)
    };

    let instr_id = instr_id_opt
        .unwrap_or_else(|| pgrx::error!("instrument not found: {}", instrument));
    let count = count_result.expect("instrument_id was Some so count_result must be Some");
    let count = count.unwrap_or_else(|e| pgrx::error!("{e}"));

    Spi::run(&format!(
        "UPDATE clob.orders \
         SET remaining = 0::numeric, status = 'cancelled', updated_at = now() \
         WHERE instrument_id = {} \
           AND participant_id = {} \
           AND status IN ('open', 'partially_filled')",
        instr_id,
        text_sql(participant),
    ))
    .unwrap_or_else(|e| pgrx::error!("failed to persist mass cancel: {e}"));

    count as i32
}

// ---------------------------------------------------------------------------
// halt / resume
// ---------------------------------------------------------------------------

#[pg_extern]
fn clob_halt_instrument(instrument: &str) {
    let instr_id = match {
        let mut engine = crate::state::lock_engine();
        engine.instrument_id(instrument).map(|id| {
            let r = engine.halt_instrument(id);
            (id, r)
        })
    } {
        Some((id, Ok(()))) => id,
        Some((_, Err(e))) => pgrx::error!("{e}"),
        None => pgrx::error!("instrument not found: {}", instrument),
    };

    Spi::run(&format!(
        "UPDATE clob.instruments SET status = 'halted' WHERE id = {}",
        instr_id
    ))
    .unwrap_or_else(|e| pgrx::warning!("failed to update instrument status: {e}"));
}

#[pg_extern]
fn clob_resume_instrument(instrument: &str) {
    let instr_id = match {
        let mut engine = crate::state::lock_engine();
        engine.instrument_id(instrument).map(|id| {
            let r = engine.resume_instrument(id);
            (id, r)
        })
    } {
        Some((id, Ok(()))) => id,
        Some((_, Err(e))) => pgrx::error!("{e}"),
        None => pgrx::error!("instrument not found: {}", instrument),
    };

    Spi::run(&format!(
        "UPDATE clob.instruments SET status = 'active' WHERE id = {}",
        instr_id
    ))
    .unwrap_or_else(|e| pgrx::warning!("failed to update instrument status: {e}"));
}

// ---------------------------------------------------------------------------
// snapshot_book — persist current book state to clob.book_snapshots
// ---------------------------------------------------------------------------

#[pg_extern]
fn clob_snapshot_book(instrument: &str) {
    let (instr_id, depths) = match {
        let engine = state::lock_engine();
        engine.instrument_id(instrument).map(|id| {
            let d = engine.get_book_depth(id, 500);
            (id, d)
        })
    } {
        Some((id, Ok(depth))) => (id, depth),
        Some((_, Err(e))) => pgrx::error!("{e}"),
        None => pgrx::error!("instrument not found: {}", instrument),
    };

    for bid in &depths.bids {
        Spi::run(&format!(
            "INSERT INTO clob.book_snapshots (instrument_id, side, price, qty, order_count) \
             VALUES ({}, 'buy', {}::numeric, {}::numeric, {})",
            instr_id, bid.price, bid.qty, bid.order_count
        ))
        .unwrap_or_else(|e| pgrx::warning!("failed to insert book snapshot: {e}"));
    }

    for ask in &depths.asks {
        Spi::run(&format!(
            "INSERT INTO clob.book_snapshots (instrument_id, side, price, qty, order_count) \
             VALUES ({}, 'sell', {}::numeric, {}::numeric, {})",
            instr_id, ask.price, ask.qty, ask.order_count
        ))
        .unwrap_or_else(|e| pgrx::warning!("failed to insert book snapshot: {e}"));
    }
}

// ---------------------------------------------------------------------------
// Integration tests
// ---------------------------------------------------------------------------

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgrx::prelude::*;

    #[pg_test]
    fn test_smoke() {
        Spi::run("SELECT 1").unwrap();
    }

    #[pg_test]
    fn test_create_instrument() {
        let id = crate::clob_create_instrument("TEST-BTC", 0.01, 1.0, 100000);
        assert!(id > 0);
    }

    #[pg_test]
    fn test_place_simple_order() {
        crate::clob_create_instrument("TEST-ETH", 0.01, 1.0, 100000);
        Spi::run(
            "INSERT INTO clob.participants (id, display_name) \
             VALUES ('alice', 'Alice') ON CONFLICT (id) DO NOTHING",
        )
        .unwrap();

        crate::clob_place_order(
            "TEST-ETH", "buy", "limit", 5.0, "alice", Some(100.0), Some("cancel_newest"),
        );
    }

    #[pg_test]
    fn test_cancel_order_returns_false_for_unknown() {
        let ok = crate::clob_cancel_order("550e8400-e29b-41d4-a716-446655440000");
        assert!(!ok);
    }

    #[pg_test]
    fn test_get_book_empty() {
        crate::clob_create_instrument("TEST-SOL", 0.01, 1.0, 100000);
        let _ = crate::clob_get_book("TEST-SOL", 10);
    }

    #[pg_test]
    fn test_halt_and_resume() {
        crate::clob_create_instrument("TEST-HALT", 0.01, 1.0, 100000);
        crate::clob_halt_instrument("TEST-HALT");
        crate::clob_resume_instrument("TEST-HALT");
    }

    #[pg_test]
    fn test_snapshot_book() {
        crate::clob_create_instrument("TEST-SNAP2", 0.01, 1.0, 100000);
        Spi::run(
            "INSERT INTO clob.participants (id, display_name) \
             VALUES ('bob2', 'Bob2') ON CONFLICT (id) DO NOTHING",
        )
        .unwrap();
        crate::clob_place_order(
            "TEST-SNAP2", "buy", "limit", 3.0, "bob2", Some(99.0), Some("cancel_newest"),
        );
        crate::clob_place_order(
            "TEST-SNAP2", "sell", "limit", 2.0, "bob2", Some(101.0), Some("cancel_newest"),
        );
        crate::clob_snapshot_book("TEST-SNAP2");

        let count: Option<i64> = Spi::get_one(
            "SELECT COUNT(*) FROM clob.book_snapshots \
             WHERE instrument_id = (SELECT id FROM clob.instruments WHERE symbol = 'TEST-SNAP2')",
        )
        .unwrap();
        assert_eq!(count, Some(2));
    }

    #[pg_test]
    fn test_get_open_orders() {
        crate::clob_create_instrument("TEST-OO", 0.01, 1.0, 100000);
        Spi::run(
            "INSERT INTO clob.participants (id, display_name) \
             VALUES ('carol', 'Carol') ON CONFLICT (id) DO NOTHING",
        )
        .unwrap();
        crate::clob_place_order(
            "TEST-OO", "buy", "limit", 5.0, "carol", Some(99.0), Some("cancel_newest"),
        );
        let _ = crate::clob_get_open_orders("carol", Some("TEST-OO"));
    }

    #[pg_test]
    fn test_place_and_match() {
        crate::clob_create_instrument("TEST-MATCH", 0.01, 1.0, 100000);
        Spi::run(
            "INSERT INTO clob.participants (id, display_name) \
             VALUES ('dave', 'Dave'), ('eve', 'Eve') \
             ON CONFLICT (id) DO NOTHING",
        )
        .unwrap();

        crate::clob_place_order(
            "TEST-MATCH", "sell", "limit", 5.0, "eve", Some(100.0), Some("cancel_newest"),
        );
        crate::clob_place_order(
            "TEST-MATCH", "buy", "limit", 5.0, "dave", Some(100.0), Some("cancel_newest"),
        );
    }

    #[pg_test]
    fn test_mass_cancel() {
        crate::clob_create_instrument("TEST-MASS", 0.01, 1.0, 100000);
        Spi::run(
            "INSERT INTO clob.participants (id, display_name) \
             VALUES ('frank', 'Frank') ON CONFLICT (id) DO NOTHING",
        )
        .unwrap();

        crate::clob_place_order(
            "TEST-MASS", "buy", "limit", 1.0, "frank", Some(99.0), Some("cancel_newest"),
        );
        crate::clob_place_order(
            "TEST-MASS", "buy", "limit", 2.0, "frank", Some(98.0), Some("cancel_newest"),
        );
        crate::clob_place_order(
            "TEST-MASS", "sell", "limit", 3.0, "frank", Some(101.0), Some("cancel_newest"),
        );

        let count = crate::clob_mass_cancel("frank", "TEST-MASS");
        assert_eq!(count, 3);
    }

    #[pg_test]
    fn test_order_persistence_lifecycle() {
        crate::clob_create_instrument("TEST-PERSIST", 0.01, 1.0, 100000);
        Spi::run(
            "INSERT INTO clob.participants (id, display_name) \
             VALUES ('persist_seller', 'Persist Seller'), ('persist_buyer', 'Persist Buyer') \
             ON CONFLICT (id) DO NOTHING",
        )
        .unwrap();

        crate::clob_place_order(
            "TEST-PERSIST",
            "sell",
            "limit",
            5.0,
            "persist_seller",
            Some(100.0),
            Some("cancel_newest"),
        );

        let open_count: Option<i64> = Spi::get_one(
            "SELECT COUNT(*) FROM clob.orders \
             WHERE participant_id = 'persist_seller' \
               AND status = 'open' \
               AND remaining = 5::numeric",
        )
        .unwrap();
        assert_eq!(open_count, Some(1));

        crate::clob_place_order(
            "TEST-PERSIST",
            "buy",
            "limit",
            2.0,
            "persist_buyer",
            Some(100.0),
            Some("cancel_newest"),
        );

        let partially_filled_count: Option<i64> = Spi::get_one(
            "SELECT COUNT(*) FROM clob.orders \
             WHERE participant_id = 'persist_seller' \
               AND status = 'partially_filled' \
               AND remaining = 3::numeric",
        )
        .unwrap();
        assert_eq!(partially_filled_count, Some(1));

        let buy_filled_count: Option<i64> = Spi::get_one(
            "SELECT COUNT(*) FROM clob.orders \
             WHERE participant_id = 'persist_buyer' \
               AND status = 'filled' \
               AND remaining = 0::numeric",
        )
        .unwrap();
        assert_eq!(buy_filled_count, Some(1));

        let order_id: Option<String> = Spi::get_one(
            "SELECT id::text FROM clob.orders \
             WHERE participant_id = 'persist_seller'",
        )
        .unwrap();
        let order_id = order_id.expect("persisted order id should exist");

        assert!(crate::clob_cancel_order(&order_id));

        let cancelled_count: Option<i64> = Spi::get_one(
            "SELECT COUNT(*) FROM clob.orders \
             WHERE participant_id = 'persist_seller' \
               AND status = 'cancelled' \
               AND remaining = 0::numeric",
        )
        .unwrap();
        assert_eq!(cancelled_count, Some(1));
    }

    #[pg_test]
    fn test_ioc_cancel_remainder() {
        crate::clob_create_instrument("TEST-IOCX", 0.01, 1.0, 100000);
        Spi::run(
            "INSERT INTO clob.participants (id, display_name) \
             VALUES ('grace', 'Grace'), ('heidi', 'Heidi') \
             ON CONFLICT (id) DO NOTHING",
        )
        .unwrap();

        crate::clob_place_order(
            "TEST-IOCX", "sell", "limit", 3.0, "heidi", Some(100.0), Some("cancel_newest"),
        );
        crate::clob_place_order(
            "TEST-IOCX", "buy", "ioc", 10.0, "grace", Some(100.0), Some("cancel_newest"),
        );
    }

    #[pg_test]
    fn test_fok_cancel_insufficient() {
        crate::clob_create_instrument("TEST-FOKX", 0.01, 1.0, 100000);
        Spi::run(
            "INSERT INTO clob.participants (id, display_name) \
             VALUES ('ivan', 'Ivan'), ('judy', 'Judy') \
             ON CONFLICT (id) DO NOTHING",
        )
        .unwrap();

        crate::clob_place_order(
            "TEST-FOKX", "sell", "limit", 3.0, "judy", Some(100.0), Some("cancel_newest"),
        );
        crate::clob_place_order(
            "TEST-FOKX", "buy", "fok", 10.0, "ivan", Some(100.0), Some("cancel_newest"),
        );
    }

    #[pg_test]
    fn test_stp_cancel_newest() {
        crate::clob_create_instrument("TEST-STPX", 0.01, 1.0, 100000);
        Spi::run(
            "INSERT INTO clob.participants (id, display_name) \
             VALUES ('mallory', 'Mallory') ON CONFLICT (id) DO NOTHING",
        )
        .unwrap();

        crate::clob_place_order(
            "TEST-STPX", "sell", "limit", 5.0, "mallory", Some(100.0), Some("cancel_newest"),
        );
        crate::clob_place_order(
            "TEST-STPX", "buy", "limit", 3.0, "mallory", Some(100.0), Some("cancel_newest"),
        );
    }

    #[pg_test]
    fn test_market_order_walks_book() {
        crate::clob_create_instrument("TEST-MKTX", 0.01, 1.0, 100000);
        Spi::run(
            "INSERT INTO clob.participants (id, display_name) \
             VALUES ('olivia', 'Olivia'), ('peter', 'Peter') \
             ON CONFLICT (id) DO NOTHING",
        )
        .unwrap();

        crate::clob_place_order(
            "TEST-MKTX", "sell", "limit", 2.0, "olivia", Some(100.0), Some("cancel_newest"),
        );
        crate::clob_place_order(
            "TEST-MKTX", "sell", "limit", 3.0, "olivia", Some(101.0), Some("cancel_newest"),
        );
        crate::clob_place_order(
            "TEST-MKTX", "buy", "market", 4.0, "peter", None, Some("cancel_newest"),
        );
    }

    #[pg_test]
    fn test_two_instruments_independent() {
        crate::clob_create_instrument("BTC5", 0.01, 1.0, 100000);
        crate::clob_create_instrument("ETH5", 0.01, 1.0, 100000);
        Spi::run(
            "INSERT INTO clob.participants (id, display_name) \
             VALUES ('trader_1', 'T1'), ('trader_2', 'T2') \
             ON CONFLICT (id) DO NOTHING",
        )
        .unwrap();

        crate::clob_place_order(
            "BTC5", "buy", "limit", 1.0, "trader_1", Some(100.0), Some("cancel_newest"),
        );
        crate::clob_place_order(
            "ETH5", "sell", "limit", 1.0, "trader_2", Some(30.0), Some("cancel_newest"),
        );

        let _ = crate::clob_get_book("BTC5", 5);
        let _ = crate::clob_get_book("ETH5", 5);
    }

    #[pg_test]
    fn test_participant_creation() {
        Spi::run(
            "INSERT INTO clob.participants (id, display_name) \
             VALUES ('p1', 'Participant 1') ON CONFLICT (id) DO NOTHING",
        )
        .unwrap();

        let name: Option<String> = Spi::get_one(
            "SELECT display_name FROM clob.participants WHERE id = 'p1'",
        )
        .unwrap();

        assert_eq!(name, Some("Participant 1".to_string()));
    }
}

#[cfg(test)]
pub mod pg_test {
    pub fn setup(_options: Vec<&str>) {}

    #[must_use]
    pub fn postgresql_conf_options() -> Vec<&'static str> {
        vec![]
    }
}
