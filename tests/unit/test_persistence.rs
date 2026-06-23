use pg_machida::engine::ClobEngine;
use pg_machida::persistence::{rebuild_book, InstrumentDef};
use pg_machida::types::*;
use rust_decimal::Decimal;
use uuid::Uuid;

fn dec(val: i64) -> Decimal {
    Decimal::new(val, 0)
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

#[allow(dead_code)]
fn make_order_with_participant(
    id: Uuid,
    instrument_id: u64,
    side: Side,
    price: Decimal,
    qty: Decimal,
    participant: &str,
    ts: u64,
) -> Order {
    Order::new(
        id,
        instrument_id,
        participant.into(),
        side,
        OrderType::Limit,
        Some(price),
        qty,
        ts,
        STPMode::None,
    )
}


// Single instrument rebuild — orders replayed in ts order


#[test]
fn test_rebuild_single_instrument_best_bid_ask() {
    let mut engine_a = ClobEngine::new();
    let inst_id = engine_a
        .create_instrument("BTC-USD", dec(1), dec(1), 50000)
        .unwrap();

    let o1 = make_limit_order(Uuid::new_v4(), inst_id, Side::Buy, dec(100), dec(3), 1);
    let o2 = make_limit_order(Uuid::new_v4(), inst_id, Side::Buy, dec(99), dec(5), 2);
    let o3 = make_limit_order(Uuid::new_v4(), inst_id, Side::Sell, dec(101), dec(4), 3);

    engine_a.place_order(o1).unwrap();
    engine_a.place_order(o2).unwrap();
    engine_a.place_order(o3).unwrap();

    // Extract state
    let resting = engine_a.resting_orders();
    let inst_defs = vec![InstrumentDef {
        id: inst_id,
        symbol: "BTC-USD".into(),
        tick_size: dec(1),
        lot_size: dec(1),
        max_ticks: 50000,
    }];

    // Rebuild into engine B
    let mut engine_b = ClobEngine::new();
    rebuild_book(&mut engine_b, &inst_defs, &resting).unwrap();

    assert_eq!(engine_b.instrument_count(), 1);
    assert_eq!(engine_b.order_count(), 3);
    assert_eq!(engine_b.best_bid(inst_id), Some(dec(100)));
    assert_eq!(engine_b.best_ask(inst_id), Some(dec(101)));
}


// Multiple instruments rebuild — each book is independent


#[test]
fn test_rebuild_multiple_instruments_independent() {
    let mut engine_a = ClobEngine::new();
    let btc_id = engine_a
        .create_instrument("BTC-USD", dec(1), dec(1), 50000)
        .unwrap();
    let eth_id = engine_a
        .create_instrument("ETH-USD", dec(1), dec(1), 50000)
        .unwrap();

    engine_a
        .place_order(make_limit_order(
            Uuid::new_v4(),
            btc_id,
            Side::Buy,
            dec(100),
            dec(2),
            1,
        ))
        .unwrap();
    engine_a
        .place_order(make_limit_order(
            Uuid::new_v4(),
            eth_id,
            Side::Buy,
            dec(30),
            dec(5),
            2,
        ))
        .unwrap();
    engine_a
        .place_order(make_limit_order(
            Uuid::new_v4(),
            eth_id,
            Side::Sell,
            dec(31),
            dec(3),
            3,
        ))
        .unwrap();

    let resting = engine_a.resting_orders();
    let inst_defs = vec![
        InstrumentDef {
            id: btc_id,
            symbol: "BTC-USD".into(),
            tick_size: dec(1),
            lot_size: dec(1),
            max_ticks: 50000,
        },
        InstrumentDef {
            id: eth_id,
            symbol: "ETH-USD".into(),
            tick_size: dec(1),
            lot_size: dec(1),
            max_ticks: 50000,
        },
    ];

    let mut engine_b = ClobEngine::new();
    rebuild_book(&mut engine_b, &inst_defs, &resting).unwrap();

    assert_eq!(engine_b.instrument_count(), 2);
    assert_eq!(engine_b.order_count(), 3);
    assert_eq!(engine_b.best_bid(btc_id), Some(dec(100)));
    assert_eq!(engine_b.best_ask(btc_id), None);
    assert_eq!(engine_b.best_bid(eth_id), Some(dec(30)));
    assert_eq!(engine_b.best_ask(eth_id), Some(dec(31)));
}


// Rebuild with partially filled orders — remaining qty preserved


#[test]
fn test_rebuild_preserves_partial_fill_state() {
    let mut engine_a = ClobEngine::new();
    let inst_id = engine_a
        .create_instrument("BTC-USD", dec(1), dec(1), 50000)
        .unwrap();

    // Resting sell at 100 with qty 10
    let sell_id = Uuid::new_v4();
    engine_a
        .place_order(make_limit_order(
            sell_id,
            inst_id,
            Side::Sell,
            dec(100),
            dec(10),
            1,
        ))
        .unwrap();

    // Aggressive buy for 4 → partially fills the sell, 6 remaining
    engine_a
        .place_order(make_limit_order(
            Uuid::new_v4(),
            inst_id,
            Side::Buy,
            dec(100),
            dec(4),
            2,
        ))
        .unwrap();

    // The sell should have 6 remaining
    assert_eq!(engine_a.order_count(), 1);

    let resting = engine_a.resting_orders();
    assert_eq!(resting.len(), 1);
    assert_eq!(resting[0].id, sell_id);
    assert_eq!(resting[0].remaining, dec(6));

    let inst_defs = vec![InstrumentDef {
        id: inst_id,
        symbol: "BTC-USD".into(),
        tick_size: dec(1),
        lot_size: dec(1),
        max_ticks: 50000,
    }];

    let mut engine_b = ClobEngine::new();
    rebuild_book(&mut engine_b, &inst_defs, &resting).unwrap();

    assert_eq!(engine_b.order_count(), 1);
    assert_eq!(engine_b.best_ask(inst_id), Some(dec(100)));

    // Verify the restored order has correct remaining qty
    let depth = engine_b.get_book_depth(inst_id, 1).unwrap();
    assert_eq!(depth.asks.len(), 1);
    assert_eq!(depth.asks[0].qty, dec(6));
    assert_eq!(depth.asks[0].order_count, 1);
}


// Rebuild after cancellations — cancelled orders not replayed


#[test]
fn test_rebuild_excludes_cancelled_orders() {
    let mut engine_a = ClobEngine::new();
    let inst_id = engine_a
        .create_instrument("BTC-USD", dec(1), dec(1), 50000)
        .unwrap();

    let keep_id = Uuid::new_v4();
    let cancel_id = Uuid::new_v4();

    engine_a
        .place_order(make_limit_order(
            keep_id,
            inst_id,
            Side::Buy,
            dec(100),
            dec(5),
            1,
        ))
        .unwrap();
    engine_a
        .place_order(make_limit_order(
            cancel_id,
            inst_id,
            Side::Buy,
            dec(99),
            dec(3),
            2,
        ))
        .unwrap();

    assert_eq!(engine_a.order_count(), 2);

    engine_a.cancel_order(cancel_id).unwrap();
    assert_eq!(engine_a.order_count(), 1);

    let resting = engine_a.resting_orders();
    assert_eq!(resting.len(), 1);
    assert_eq!(resting[0].id, keep_id);

    let inst_defs = vec![InstrumentDef {
        id: inst_id,
        symbol: "BTC-USD".into(),
        tick_size: dec(1),
        lot_size: dec(1),
        max_ticks: 50000,
    }];

    let mut engine_b = ClobEngine::new();
    rebuild_book(&mut engine_b, &inst_defs, &resting).unwrap();

    assert_eq!(engine_b.order_count(), 1);
    assert_eq!(engine_b.best_bid(inst_id), Some(dec(100)));

    // Verify cancelled order is NOT on the rebuilt book
    let depth = engine_b.get_book_depth(inst_id, 5).unwrap();
    let has_99 = depth.bids.iter().any(|l| l.price == dec(99));
    assert!(!has_99, "cancelled order at 99 must not appear in rebuilt book");
}


// Time priority preserved — earlier ts fills first after rebuild


#[test]
fn test_rebuild_preserves_time_priority() {
    let mut engine_a = ClobEngine::new();
    let inst_id = engine_a
        .create_instrument("BTC-USD", dec(1), dec(1), 50000)
        .unwrap();

    let s1_id = Uuid::new_v4();
    let s2_id = Uuid::new_v4();
    let s3_id = Uuid::new_v4();

    engine_a
        .place_order(make_limit_order(
            s1_id,
            inst_id,
            Side::Sell,
            dec(100),
            dec(3),
            100,
        ))
        .unwrap();
    engine_a
        .place_order(make_limit_order(
            s2_id,
            inst_id,
            Side::Sell,
            dec(100),
            dec(5),
            200,
        ))
        .unwrap();
    engine_a
        .place_order(make_limit_order(
            s3_id,
            inst_id,
            Side::Sell,
            dec(100),
            dec(2),
            300,
        ))
        .unwrap();

    let resting = engine_a.resting_orders();
    let inst_defs = vec![InstrumentDef {
        id: inst_id,
        symbol: "BTC-USD".into(),
        tick_size: dec(1),
        lot_size: dec(1),
        max_ticks: 50000,
    }];

    // Rebuild engine B and place an aggressive buy
    let mut engine_b = ClobEngine::new();
    rebuild_book(&mut engine_b, &inst_defs, &resting).unwrap();

    let buy = Order::new(
        Uuid::new_v4(),
        inst_id,
        "trader_b".into(),
        Side::Buy,
        OrderType::Limit,
        Some(dec(100)),
        dec(6),
        400,
        STPMode::None,
    );
    let result = engine_b.place_order(buy).unwrap();

    assert_eq!(result.status, OrderStatus::Filled);
    assert_eq!(result.filled_qty, dec(6));
    assert_eq!(result.trades.len(), 2);

    // First trade should be against s1 (earliest ts=100), then s2 (ts=200)
    assert_eq!(result.trades[0].qty, dec(3));
    assert_eq!(result.trades[0].sell_order_id, s1_id);
    assert_eq!(result.trades[1].qty, dec(3));
    assert_eq!(result.trades[1].sell_order_id, s2_id);
}


// Rebuild from empty book


#[test]
fn test_rebuild_empty_book() {
    let mut engine_a = ClobEngine::new();
    let inst_id = engine_a
        .create_instrument("BTC-USD", dec(1), dec(1), 50000)
        .unwrap();

    let resting = engine_a.resting_orders();
    assert!(resting.is_empty());

    let inst_defs = vec![InstrumentDef {
        id: inst_id,
        symbol: "BTC-USD".into(),
        tick_size: dec(1),
        lot_size: dec(1),
        max_ticks: 50000,
    }];

    let mut engine_b = ClobEngine::new();
    rebuild_book(&mut engine_b, &inst_defs, &resting).unwrap();

    assert_eq!(engine_b.instrument_count(), 1);
    assert_eq!(engine_b.order_count(), 0);
    assert_eq!(engine_b.best_bid(inst_id), None);
    assert_eq!(engine_b.best_ask(inst_id), None);
}


// rebuild_book errors — invalid instrument


#[test]
fn test_rebuild_book_unknown_instrument_in_order() {
    let mut engine = ClobEngine::new();

    // Order references instrument 1, but no instrument created
    let order = make_limit_order(Uuid::new_v4(), 1, Side::Buy, dec(100), dec(5), 1);

    let result = rebuild_book(&mut engine, &[], &[order]);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("instrument not found"));
}


// Idempotent trade IDs — same inputs → same UUID


#[test]
fn test_trade_id_idempotent() {
    let buy_id = Uuid::new_v4();
    let sell_id = Uuid::new_v4();

    let id1 = Trade::compute_id(1, buy_id, sell_id, dec(100), dec(5), 0);
    let id2 = Trade::compute_id(1, buy_id, sell_id, dec(100), dec(5), 0);
    assert_eq!(id1, id2, "same inputs must produce same trade ID");
}

#[test]
fn test_trade_id_different_fill_seq() {
    let buy_id = Uuid::new_v4();
    let sell_id = Uuid::new_v4();

    let id1 = Trade::compute_id(1, buy_id, sell_id, dec(100), dec(5), 0);
    let id2 = Trade::compute_id(1, buy_id, sell_id, dec(100), dec(5), 1);
    assert_ne!(id1, id2, "different fill_seq must produce different trade ID");
}

#[test]
fn test_trade_id_different_instruments() {
    let buy_id = Uuid::new_v4();
    let sell_id = Uuid::new_v4();

    let id1 = Trade::compute_id(1, buy_id, sell_id, dec(100), dec(5), 0);
    let id2 = Trade::compute_id(2, buy_id, sell_id, dec(100), dec(5), 0);
    assert_ne!(id1, id2, "different instrument must produce different trade ID");
}

#[test]
fn test_trade_id_different_qty() {
    let buy_id = Uuid::new_v4();
    let sell_id = Uuid::new_v4();

    let id1 = Trade::compute_id(1, buy_id, sell_id, dec(100), dec(5), 0);
    let id2 = Trade::compute_id(1, buy_id, sell_id, dec(100), dec(3), 0);
    assert_ne!(id1, id2, "different qty must produce different trade ID");
}


// Rebuild preserves depth across multiple price levels


#[test]
fn test_rebuild_preserves_depth() {
    let mut engine_a = ClobEngine::new();
    let inst_id = engine_a
        .create_instrument("BTC-USD", dec(1), dec(1), 50000)
        .unwrap();

    engine_a
        .place_order(make_limit_order(
            Uuid::new_v4(),
            inst_id,
            Side::Buy,
            dec(100),
            dec(3),
            1,
        ))
        .unwrap();
    engine_a
        .place_order(make_limit_order(
            Uuid::new_v4(),
            inst_id,
            Side::Buy,
            dec(99),
            dec(2),
            2,
        ))
        .unwrap();
    engine_a
        .place_order(make_limit_order(
            Uuid::new_v4(),
            inst_id,
            Side::Sell,
            dec(101),
            dec(4),
            3,
        ))
        .unwrap();
    engine_a
        .place_order(make_limit_order(
            Uuid::new_v4(),
            inst_id,
            Side::Sell,
            dec(102),
            dec(1),
            4,
        ))
        .unwrap();

    let depth_a = engine_a.get_book_depth(inst_id, 5).unwrap();

    let resting = engine_a.resting_orders();
    let inst_defs = vec![InstrumentDef {
        id: inst_id,
        symbol: "BTC-USD".into(),
        tick_size: dec(1),
        lot_size: dec(1),
        max_ticks: 50000,
    }];

    let mut engine_b = ClobEngine::new();
    rebuild_book(&mut engine_b, &inst_defs, &resting).unwrap();

    let depth_b = engine_b.get_book_depth(inst_id, 5).unwrap();

    assert_eq!(depth_b.bids.len(), depth_a.bids.len());
    assert_eq!(depth_b.asks.len(), depth_a.asks.len());

    for (a, b) in depth_a.bids.iter().zip(depth_b.bids.iter()) {
        assert_eq!(a.price, b.price);
        assert_eq!(a.qty, b.qty);
        assert_eq!(a.order_count, b.order_count);
    }
    for (a, b) in depth_a.asks.iter().zip(depth_b.asks.iter()) {
        assert_eq!(a.price, b.price);
        assert_eq!(a.qty, b.qty);
        assert_eq!(a.order_count, b.order_count);
    }
}
