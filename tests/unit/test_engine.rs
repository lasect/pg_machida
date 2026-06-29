use pg_machida::engine::*;
use pg_machida::error::ClobError;
use pg_machida::types::*;
use rust_decimal::Decimal;
use uuid::Uuid;

fn dec(val: i64) -> Decimal {
    Decimal::new(val, 0)
}

fn dec_f(val: i64, scale: u32) -> Decimal {
    Decimal::new(val, scale)
}

fn make_limit_order(
    id: Uuid,
    instrument_id: u64,
    side: Side,
    price: Decimal,
    qty: Decimal,
    ts: u64,
) -> Order {
    Order::new(
        id,
        instrument_id,
        "trader_a".into(),
        side,
        OrderType::Limit,
        Some(price),
        qty,
        ts,
        STPMode::None,
    )
}

fn make_market_order(id: Uuid, instrument_id: u64, side: Side, qty: Decimal, ts: u64) -> Order {
    Order::new(
        id,
        instrument_id,
        "trader_a".into(),
        side,
        OrderType::Market,
        None,
        qty,
        ts,
        STPMode::None,
    )
}


// create_instrument — happy path


#[test]
fn test_create_instrument() {
    let mut engine = ClobEngine::new();
    let id = engine
        .create_instrument("BTC-USD", dec_f(1, 2), dec(1), 50000)
        .expect("should create instrument");
    assert_eq!(id, 1);
    assert_eq!(engine.instrument_count(), 1);
    assert_eq!(engine.instrument_id("BTC-USD"), Some(1));
}

#[test]
fn test_create_multiple_instruments_distinct_ids() {
    let mut engine = ClobEngine::new();
    let id1 = engine
        .create_instrument("BTC-USD", dec_f(1, 2), dec(1), 50000)
        .unwrap();
    let id2 = engine
        .create_instrument("ETH-USD", dec_f(1, 2), dec(1), 50000)
        .unwrap();
    assert_ne!(id1, id2);
    assert_eq!(engine.instrument_count(), 2);
    assert_eq!(engine.instrument_id("BTC-USD"), Some(id1));
    assert_eq!(engine.instrument_id("ETH-USD"), Some(id2));
}


// create_instrument — errors


#[test]
fn test_create_instrument_empty_symbol() {
    let mut engine = ClobEngine::new();
    let err = engine
        .create_instrument("", dec_f(1, 2), dec(1), 50000)
        .unwrap_err();
    assert!(err.to_string().contains("empty"));
}

#[test]
fn test_create_instrument_duplicate_symbol() {
    let mut engine = ClobEngine::new();
    engine
        .create_instrument("BTC-USD", dec_f(1, 2), dec(1), 50000)
        .unwrap();
    let err = engine
        .create_instrument("BTC-USD", dec_f(1, 2), dec(1), 50000)
        .unwrap_err();
    assert!(err.to_string().contains("already exists"));
}

#[test]
fn test_create_instrument_zero_tick_size() {
    let mut engine = ClobEngine::new();
    let err = engine
        .create_instrument("BTC-USD", Decimal::ZERO, dec(1), 50000)
        .unwrap_err();
    assert!(err.to_string().contains("tick_size"));
}

#[test]
fn test_create_instrument_zero_lot_size() {
    let mut engine = ClobEngine::new();
    let err = engine
        .create_instrument("BTC-USD", dec_f(1, 2), Decimal::ZERO, 50000)
        .unwrap_err();
    assert!(err.to_string().contains("lot_size"));
}

#[test]
fn test_create_instrument_fractional_lot_size() {
    let mut engine = ClobEngine::new();
    let err = engine
        .create_instrument("BTC-USD", dec_f(1, 2), dec_f(5, 1), 50000)
        .unwrap_err();
    assert!(err.to_string().contains("whole number"));
}

#[test]
fn test_create_instrument_zero_max_ticks() {
    let mut engine = ClobEngine::new();
    let err = engine
        .create_instrument("BTC-USD", dec_f(1, 2), dec(1), 0)
        .unwrap_err();
    assert!(err.to_string().contains("max_ticks"));
}


// place_order — happy path


#[test]
fn test_place_limit_order_rests_on_book() {
    let mut engine = ClobEngine::new();
    let instr_id = engine
        .create_instrument("BTC-USD", dec_f(1, 2), dec(1), 20000)
        .unwrap();

    let order_id = Uuid::new_v4();
    let order = make_limit_order(order_id, instr_id, Side::Buy, dec(100), dec(5), 1);
    let result = engine.place_order(order).unwrap();

    assert_eq!(result.status, OrderStatus::Open);
    assert_eq!(result.filled_qty, dec(0));
    assert_eq!(engine.order_count(), 1);
    assert_eq!(engine.best_bid(instr_id), Some(dec(100)));
}

#[test]
fn test_place_order_matches_across_two_instruments_independent_books() {
    let mut engine = ClobEngine::new();
    let btc_id = engine
        .create_instrument("BTC-USD", dec_f(1, 2), dec(1), 20000)
        .unwrap();
    let eth_id = engine
        .create_instrument("ETH-USD", dec_f(1, 2), dec(1), 20000)
        .unwrap();

    // Insert resting sell on BTC book
    engine
        .place_order(make_limit_order(
            Uuid::new_v4(),
            btc_id,
            Side::Sell,
            dec(100),
            dec(10),
            1,
        ))
        .unwrap();

    // Place buy on BTC — should match against BTC's sell, not ETH's
    let buy = make_limit_order(Uuid::new_v4(), btc_id, Side::Buy, dec(100), dec(5), 2);
    let result = engine.place_order(buy).unwrap();

    assert_eq!(result.status, OrderStatus::Filled);
    assert_eq!(result.filled_qty, dec(5));

    // ETH book should still be empty
    assert_eq!(engine.best_bid(eth_id), None);
    assert_eq!(engine.best_ask(eth_id), None);
}


// place_order — errors


#[test]
fn test_place_order_nonexistent_instrument() {
    let mut engine = ClobEngine::new();
    let order = make_limit_order(Uuid::new_v4(), 999, Side::Buy, dec(100), dec(5), 1);
    let err = engine.place_order(order).unwrap_err();
    assert!(matches!(err, ClobError::InstrumentNotFound(_)));
}

#[test]
fn test_place_order_tick_size_violation() {
    let mut engine = ClobEngine::new();
    let instr_id = engine
        .create_instrument("BTC-USD", dec_f(5, 2), dec(1), 20000)
        .unwrap();

    // tick_size = 0.05, price 100.02 is not a multiple of 0.05
    let order = make_limit_order(
        Uuid::new_v4(),
        instr_id,
        Side::Buy,
        dec_f(10002, 2),
        dec(5),
        1,
    );
    let err = engine.place_order(order).unwrap_err();
    assert!(matches!(err, ClobError::InvalidPrice(_)));
    assert!(err.to_string().contains("tick_size"));
}

#[test]
fn test_place_order_tick_size_ok() {
    let mut engine = ClobEngine::new();
    let instr_id = engine
        .create_instrument("BTC-USD", dec_f(5, 2), dec(1), 20000)
        .unwrap();

    // tick_size = 0.05, price 100.05 is a valid tick
    let order = make_limit_order(
        Uuid::new_v4(),
        instr_id,
        Side::Buy,
        dec_f(10005, 2),
        dec(5),
        1,
    );
    let result = engine.place_order(order).unwrap();
    assert_eq!(result.status, OrderStatus::Open);
}

#[test]
fn test_place_order_lot_size_violation() {
    let mut engine = ClobEngine::new();
    let instr_id = engine
        .create_instrument("BTC-USD", dec_f(1, 2), dec(5), 20000)
        .unwrap();

    // lot_size = 5, qty 7 is not a multiple of 5
    let order = make_limit_order(Uuid::new_v4(), instr_id, Side::Buy, dec(100), dec(7), 1);
    let err = engine.place_order(order).unwrap_err();
    assert!(matches!(err, ClobError::InvalidQty(_)));
    assert!(err.to_string().contains("lot_size"));
}

#[test]
fn test_place_order_lot_size_ok() {
    let mut engine = ClobEngine::new();
    let instr_id = engine
        .create_instrument("BTC-USD", dec_f(1, 2), dec(5), 20000)
        .unwrap();

    // lot_size = 5, qty = 10 is a valid lot
    let order = make_limit_order(Uuid::new_v4(), instr_id, Side::Buy, dec(100), dec(10), 1);
    let result = engine.place_order(order).unwrap();
    assert_eq!(result.status, OrderStatus::Open);
}

#[test]
fn test_place_order_on_halted_instrument() {
    let mut engine = ClobEngine::new();
    let instr_id = engine
        .create_instrument("BTC-USD", dec_f(1, 2), dec(1), 20000)
        .unwrap();

    engine.halt_instrument(instr_id).unwrap();

    let order = make_limit_order(Uuid::new_v4(), instr_id, Side::Buy, dec(100), dec(5), 1);
    let err = engine.place_order(order).unwrap_err();
    assert!(matches!(err, ClobError::InstrumentHalted(_)));
}

#[test]
fn test_market_order_skips_tick_size_validation() {
    let mut engine = ClobEngine::new();
    let instr_id = engine
        .create_instrument("BTC-USD", dec_f(5, 2), dec(1), 20000)
        .unwrap();

    // Insert a resting sell at 100
    engine
        .place_order(make_limit_order(
            Uuid::new_v4(),
            instr_id,
            Side::Sell,
            dec(100),
            dec(5),
            1,
        ))
        .unwrap();

    // Market buy — no price, so tick_size is irrelevant
    let buy = make_market_order(Uuid::new_v4(), instr_id, Side::Buy, dec(3), 2);
    let result = engine.place_order(buy).unwrap();
    assert_eq!(result.status, OrderStatus::Filled);
    assert_eq!(result.filled_qty, dec(3));
}


// cancel_order


#[test]
fn test_cancel_order_removes_from_engine_index() {
    let mut engine = ClobEngine::new();
    let instr_id = engine
        .create_instrument("BTC-USD", dec_f(1, 2), dec(1), 20000)
        .unwrap();

    let order_id = Uuid::new_v4();
    engine
        .place_order(make_limit_order(
            order_id,
            instr_id,
            Side::Buy,
            dec(100),
            dec(5),
            1,
        ))
        .unwrap();

    assert_eq!(engine.order_count(), 1);

    engine.cancel_order(order_id).unwrap();

    assert_eq!(engine.order_count(), 0);
    assert_eq!(engine.best_bid(instr_id), None);
}

#[test]
fn test_cancel_nonexistent_order() {
    let mut engine = ClobEngine::new();
    engine
        .create_instrument("BTC-USD", dec_f(1, 2), dec(1), 20000)
        .unwrap();

    let err = engine.cancel_order(Uuid::new_v4()).unwrap_err();
    assert!(matches!(err, ClobError::OrderNotFound(_)));
}


// get_book_depth


#[test]
fn test_get_book_depth() {
    let mut engine = ClobEngine::new();
    let instr_id = engine
        .create_instrument("BTC-USD", dec_f(1, 2), dec(1), 20000)
        .unwrap();

    engine
        .place_order(make_limit_order(
            Uuid::new_v4(),
            instr_id,
            Side::Buy,
            dec(99),
            dec(3),
            1,
        ))
        .unwrap();
    engine
        .place_order(make_limit_order(
            Uuid::new_v4(),
            instr_id,
            Side::Sell,
            dec(101),
            dec(5),
            2,
        ))
        .unwrap();

    let depth = engine.get_book_depth(instr_id, 5).unwrap();

    assert_eq!(depth.bids.len(), 1);
    assert_eq!(depth.bids[0].price, dec(99));
    assert_eq!(depth.bids[0].qty, dec(3));

    assert_eq!(depth.asks.len(), 1);
    assert_eq!(depth.asks[0].price, dec(101));
    assert_eq!(depth.asks[0].qty, dec(5));
}

#[test]
fn test_get_book_depth_nonexistent_instrument() {
    let engine = ClobEngine::new();
    let err = engine.get_book_depth(999, 5).unwrap_err();
    assert!(matches!(err, ClobError::InstrumentNotFound(_)));
}


// halt / resume


#[test]
fn test_halt_and_resume() {
    let mut engine = ClobEngine::new();
    let instr_id = engine
        .create_instrument("BTC-USD", dec_f(1, 2), dec(1), 20000)
        .unwrap();

    assert_eq!(
        engine.instrument_status(instr_id),
        Some(InstrumentStatus::Active)
    );

    engine.halt_instrument(instr_id).unwrap();
    assert_eq!(
        engine.instrument_status(instr_id),
        Some(InstrumentStatus::Halted)
    );

    engine.resume_instrument(instr_id).unwrap();
    assert_eq!(
        engine.instrument_status(instr_id),
        Some(InstrumentStatus::Active)
    );
}

#[test]
fn test_halt_nonexistent_instrument() {
    let mut engine = ClobEngine::new();
    let err = engine.halt_instrument(999).unwrap_err();
    assert!(matches!(err, ClobError::InstrumentNotFound(_)));
}

#[test]
fn test_resume_nonexistent_instrument() {
    let mut engine = ClobEngine::new();
    let err = engine.resume_instrument(999).unwrap_err();
    assert!(matches!(err, ClobError::InstrumentNotFound(_)));
}


// circuit breaker


#[test]
fn test_circuit_breaker_triggers_halt_on_large_move() {
    let mut engine = ClobEngine::new();
    let instr_id = engine
        .create_instrument("BTC-USD", dec_f(1, 2), dec(1), 20000)
        .unwrap();

    // Set circuit breaker: reference = 100, halt at 10% move
    engine
        .set_circuit_breaker(
            instr_id,
            CircuitBreaker {
                reference_price: dec(100),
                halt_pct: dec_f(1, 1), // 0.1 = 10%
                cooldown_secs: 300,
            },
        )
        .unwrap();

    // Place resting sell at 100
    engine
        .place_order(make_limit_order(
            Uuid::new_v4(),
            instr_id,
            Side::Sell,
            dec(100),
            dec(1),
            1,
        ))
        .unwrap();

    // Place resting sell at 115 (>10% from reference 100)
    engine
        .place_order(make_limit_order(
            Uuid::new_v4(),
            instr_id,
            Side::Sell,
            dec(115),
            dec(1),
            2,
        ))
        .unwrap();

    // Aggressive buy at 115 — will match at 100 first, then 115
    let buy = make_limit_order(Uuid::new_v4(), instr_id, Side::Buy, dec(115), dec(2), 3);
    let result = engine.place_order(buy).unwrap();

    assert_eq!(result.status, OrderStatus::Filled);
    assert_eq!(result.trades.len(), 2);
    assert_eq!(result.trades[0].price, dec(100));
    assert_eq!(result.trades[1].price, dec(115));

    // The trade at 115 breaches the circuit breaker (15% > 10%)
    assert_eq!(
        engine.instrument_status(instr_id),
        Some(InstrumentStatus::Halted)
    );

    // Subsequent orders should be rejected
    let next = make_limit_order(Uuid::new_v4(), instr_id, Side::Buy, dec(100), dec(1), 4);
    let err = engine.place_order(next).unwrap_err();
    assert!(matches!(err, ClobError::InstrumentHalted(_)));
}

#[test]
fn test_circuit_breaker_not_triggered_on_small_move() {
    let mut engine = ClobEngine::new();
    let instr_id = engine
        .create_instrument("BTC-USD", dec_f(1, 2), dec(1), 20000)
        .unwrap();

    engine
        .set_circuit_breaker(
            instr_id,
            CircuitBreaker {
                reference_price: dec(100),
                halt_pct: dec_f(1, 1), // 10%
                cooldown_secs: 300,
            },
        )
        .unwrap();

    // Sell at 100
    engine
        .place_order(make_limit_order(
            Uuid::new_v4(),
            instr_id,
            Side::Sell,
            dec(100),
            dec(1),
            1,
        ))
        .unwrap();

    // Buy at 100 — no breach
    let buy = make_limit_order(Uuid::new_v4(), instr_id, Side::Buy, dec(100), dec(1), 2);
    let result = engine.place_order(buy).unwrap();

    assert_eq!(result.status, OrderStatus::Filled);
    assert_eq!(
        engine.instrument_status(instr_id),
        Some(InstrumentStatus::Active)
    );
}

#[test]
fn test_circuit_breaker_no_trigger_without_cb_configured() {
    let mut engine = ClobEngine::new();
    let instr_id = engine
        .create_instrument("BTC-USD", dec_f(1, 2), dec(1), 50000)
        .unwrap();

    // No circuit breaker set

    engine
        .place_order(make_limit_order(
            Uuid::new_v4(),
            instr_id,
            Side::Sell,
            dec(100),
            dec(1),
            1,
        ))
        .unwrap();
    engine
        .place_order(make_limit_order(
            Uuid::new_v4(),
            instr_id,
            Side::Sell,
            dec(150),
            dec(1),
            2,
        ))
        .unwrap();

    let buy = make_limit_order(Uuid::new_v4(), instr_id, Side::Buy, dec(150), dec(2), 3);
    let result = engine.place_order(buy).unwrap();
    assert_eq!(result.status, OrderStatus::Filled);
    assert_eq!(
        engine.instrument_status(instr_id),
        Some(InstrumentStatus::Active)
    );
}


// Two instruments are independent


#[test]
fn test_two_instruments_independent_tick_arrays() {
    let mut engine = ClobEngine::new();
    let btc_id = engine
        .create_instrument("BTC-USD", dec_f(1, 2), dec(1), 50000)
        .unwrap();
    let eth_id = engine
        .create_instrument("ETH-USD", dec_f(1, 2), dec(1), 50000)
        .unwrap();

    // Place orders on BTC
    engine
        .place_order(make_limit_order(
            Uuid::new_v4(),
            btc_id,
            Side::Buy,
            dec(100),
            dec(1),
            1,
        ))
        .unwrap();
    engine
        .place_order(make_limit_order(
            Uuid::new_v4(),
            btc_id,
            Side::Sell,
            dec(110),
            dec(2),
            2,
        ))
        .unwrap();

    // Place orders on ETH (different prices)
    engine
        .place_order(make_limit_order(
            Uuid::new_v4(),
            eth_id,
            Side::Buy,
            dec(30),
            dec(3),
            3,
        ))
        .unwrap();
    engine
        .place_order(make_limit_order(
            Uuid::new_v4(),
            eth_id,
            Side::Sell,
            dec(31),
            dec(4),
            4,
        ))
        .unwrap();

    // BTC and ETH books should be completely independent
    assert_eq!(engine.best_bid(btc_id), Some(dec(100)));
    assert_eq!(engine.best_ask(btc_id), Some(dec(110)));
    assert_eq!(engine.best_bid(eth_id), Some(dec(30)));
    assert_eq!(engine.best_ask(eth_id), Some(dec(31)));
    assert_eq!(engine.order_count(), 4);
}

#[test]
fn test_halt_one_instrument_does_not_affect_other() {
    let mut engine = ClobEngine::new();
    let btc_id = engine
        .create_instrument("BTC-USD", dec_f(1, 2), dec(1), 50000)
        .unwrap();
    let eth_id = engine
        .create_instrument("ETH-USD", dec_f(1, 2), dec(1), 50000)
        .unwrap();

    engine.halt_instrument(btc_id).unwrap();

    // BTC is halted
    let btc_order = make_limit_order(Uuid::new_v4(), btc_id, Side::Buy, dec(100), dec(1), 1);
    assert!(engine.place_order(btc_order).is_err());

    // ETH is still active
    let eth_order = make_limit_order(Uuid::new_v4(), eth_id, Side::Buy, dec(100), dec(1), 1);
    assert!(engine.place_order(eth_order).is_ok());
}


// Mass cancel — cancel all resting orders via cancel loop


#[test]
fn test_cancel_all_orders_clears_book() {
    let mut engine = ClobEngine::new();
    let instr_id = engine
        .create_instrument("BTC-USD", dec_f(1, 2), dec(1), 50000)
        .unwrap();

    let ids: Vec<Uuid> = (0..5)
        .map(|_| {
            let id = Uuid::new_v4();
            engine
                .place_order(make_limit_order(id, instr_id, Side::Buy, dec(100), dec(1), 1))
                .unwrap();
            id
        })
        .collect();

    assert_eq!(engine.order_count(), 5);

    for id in &ids {
        engine.cancel_order(*id).unwrap();
    }

    assert_eq!(engine.order_count(), 0);
    assert_eq!(engine.best_bid(instr_id), None);
}
