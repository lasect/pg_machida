use pg_machida::book::*;
use pg_machida::matching::match_order;
use pg_machida::types::*;
use rust_decimal::Decimal;
use uuid::Uuid;

const TEST_MAX_TICKS: usize = 50000;

fn make_limit_order(side: Side, price: Decimal, qty: Decimal, ts: u64) -> Order {
    Order::new(
        Uuid::new_v4(),
        1,
        "trader_a".into(),
        side,
        OrderType::Limit,
        Some(price),
        qty,
        ts,
        STPMode::None,
    )
}

fn make_market_order(side: Side, qty: Decimal, ts: u64) -> Order {
    Order::new(
        Uuid::new_v4(),
        1,
        "trader_a".into(),
        side,
        OrderType::Market,
        None,
        qty,
        ts,
        STPMode::None,
    )
}

fn make_ioc_order(side: Side, price: Decimal, qty: Decimal, ts: u64) -> Order {
    Order::new(
        Uuid::new_v4(),
        1,
        "trader_a".into(),
        side,
        OrderType::IOC,
        Some(price),
        qty,
        ts,
        STPMode::None,
    )
}

fn make_fok_order(side: Side, price: Decimal, qty: Decimal, ts: u64) -> Order {
    Order::new(
        Uuid::new_v4(),
        1,
        "trader_a".into(),
        side,
        OrderType::FOK,
        Some(price),
        qty,
        ts,
        STPMode::None,
    )
}

fn make_order_with_participant(
    side: Side,
    price: Decimal,
    qty: Decimal,
    participant: &str,
    ts: u64,
) -> Order {
    Order::new(
        Uuid::new_v4(),
        1,
        participant.into(),
        side,
        OrderType::Limit,
        Some(price),
        qty,
        ts,
        STPMode::None,
    )
}

fn make_stp_order(
    side: Side,
    price: Decimal,
    qty: Decimal,
    participant: &str,
    stp: STPMode,
    ts: u64,
) -> Order {
    Order::new(
        Uuid::new_v4(),
        1,
        participant.into(),
        side,
        OrderType::Limit,
        Some(price),
        qty,
        ts,
        stp,
    )
}

fn dec(val: i64) -> Decimal {
    Decimal::new(val, 0)
}

// ---------------------------------------------------------------------------
// Simple limit-limit match
// ---------------------------------------------------------------------------

#[test]
fn test_simple_limit_match_buy_aggresses_sell() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    let sell = make_limit_order(Side::Sell, dec(100), dec(10), 1);

    // Insert resting sell
    book.insert(sell);

    let buy = make_limit_order(Side::Buy, dec(100), dec(10), 2);
    let result = match_order(&mut book, buy);

    assert_eq!(result.status, OrderStatus::Filled);
    assert_eq!(result.filled_qty, dec(10));
    assert_eq!(result.trades.len(), 1);
    assert_eq!(result.trades[0].price, dec(100));
    assert_eq!(result.trades[0].qty, dec(10));
    assert_eq!(result.avg_fill_price, Some(dec(100)));

    // Tick array at price 100 should be zeroed
    let tick = price_to_tick(dec(100));
    assert_eq!(book.asks.get_qty(tick), 0);
    assert!(book.ask_levels.get(&tick).is_none());
    assert!(book.order_index.is_empty());
}

#[test]
fn test_simple_limit_match_sell_aggresses_bid() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    let buy = make_limit_order(Side::Buy, dec(100), dec(10), 1);

    // Insert resting buy
    book.insert(buy);

    let sell = make_limit_order(Side::Sell, dec(100), dec(10), 2);
    let result = match_order(&mut book, sell);

    assert_eq!(result.status, OrderStatus::Filled);
    assert_eq!(result.filled_qty, dec(10));
    assert_eq!(result.trades.len(), 1);
    assert_eq!(result.trades[0].price, dec(100));
    assert_eq!(result.trades[0].qty, dec(10));

    let tick = price_to_tick(dec(100));
    assert_eq!(book.bids.get_qty(tick), 0);
    assert!(book.bid_levels.get(&tick).is_none());
    assert!(book.order_index.is_empty());
}

// ---------------------------------------------------------------------------
// Partial fill — resting order larger than incoming
// ---------------------------------------------------------------------------

#[test]
fn test_partial_fill_resting_larger_than_incoming() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    let sell = make_limit_order(Side::Sell, dec(100), dec(10), 1);
    let sell_id = sell.id;
    book.insert(sell);

    let buy = make_limit_order(Side::Buy, dec(100), dec(3), 2);
    let result = match_order(&mut book, buy);

    assert_eq!(result.status, OrderStatus::Filled);
    assert_eq!(result.filled_qty, dec(3));
    assert_eq!(result.trades.len(), 1);
    assert_eq!(result.trades[0].qty, dec(3));

    // Resting sell should still have 7 remaining
    let tick = price_to_tick(dec(100));
    assert_eq!(book.asks.get_qty(tick), 7);
    let level = book
        .ask_levels
        .get(&tick)
        .expect("ask level should exist at tick");
    assert_eq!(level.total_qty, dec(7));
    assert_eq!(level.orders.len(), 1);
    assert_eq!(level.orders[0].remaining, dec(7));
    assert_eq!(level.orders[0].id, sell_id);
}

// ---------------------------------------------------------------------------
// Partial fill — incoming order larger than resting
// ---------------------------------------------------------------------------

#[test]
fn test_partial_fill_incoming_larger_than_resting() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    let sell = make_limit_order(Side::Sell, dec(100), dec(3), 1);
    book.insert(sell);

    let buy = make_limit_order(Side::Buy, dec(100), dec(10), 2);
    let buy_id = buy.id;
    let result = match_order(&mut book, buy);

    assert_eq!(result.status, OrderStatus::PartiallyFilled);
    assert_eq!(result.filled_qty, dec(3));
    assert_eq!(result.trades.len(), 1);
    assert_eq!(result.trades[0].qty, dec(3));

    // Resting sell should be fully consumed
    let tick = price_to_tick(dec(100));
    assert_eq!(book.asks.get_qty(tick), 0);
    assert!(book.ask_levels.get(&tick).is_none());

    // Remainder (7) of buy should be resting on book
    let bid_tick = price_to_tick(dec(100));
    assert_eq!(book.bids.get_qty(bid_tick), 7);
    let level = book
        .bid_levels
        .get(&bid_tick)
        .expect("bid level should exist at tick");
    assert_eq!(level.total_qty, dec(7));
    assert_eq!(level.orders[0].remaining, dec(7));
    assert_eq!(level.orders[0].id, buy_id);
}

// ---------------------------------------------------------------------------
// Price-time priority (FIFO within level)
// ---------------------------------------------------------------------------

#[test]
fn test_price_time_priority_fills_oldest_first() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    let sell1 = make_limit_order(Side::Sell, dec(100), dec(5), 1);
    let sell2 = make_limit_order(Side::Sell, dec(100), dec(5), 2);
    let sell1_id = sell1.id;
    let sell2_id = sell2.id;

    book.insert(sell1);
    book.insert(sell2);

    // Buy 7: should fill sell1 (5) then sell2 (2)
    let buy = make_limit_order(Side::Buy, dec(101), dec(7), 3);
    let result = match_order(&mut book, buy);

    assert_eq!(result.status, OrderStatus::Filled);
    assert_eq!(result.filled_qty, dec(7));
    assert_eq!(result.trades.len(), 2);

    // First trade should be against sell1 (earlier timestamp)
    assert_eq!(result.trades[0].qty, dec(5));
    assert_eq!(result.trades[0].sell_order_id, sell1_id);

    // Second trade against sell2 (partial fill)
    assert_eq!(result.trades[1].qty, dec(2));
    assert_eq!(result.trades[1].sell_order_id, sell2_id);

    // sell1 fully consumed, sell2 has 3 remaining
    let tick = price_to_tick(dec(100));
    assert_eq!(book.asks.get_qty(tick), 3);
    let level = book
        .ask_levels
        .get(&tick)
        .expect("ask level should exist at tick");
    assert_eq!(level.orders.len(), 1);
    assert_eq!(level.orders[0].id, sell2_id);
    assert_eq!(level.orders[0].remaining, dec(3));
}

// ---------------------------------------------------------------------------
// Market order walks multiple levels
// ---------------------------------------------------------------------------

#[test]
fn test_market_order_walks_multiple_levels() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);

    book.insert(make_limit_order(Side::Sell, dec(100), dec(2), 1));
    book.insert(make_limit_order(Side::Sell, dec(101), dec(3), 2));
    book.insert(make_limit_order(Side::Sell, dec(102), dec(4), 3));

    let buy = make_market_order(Side::Buy, dec(7), 4);
    let result = match_order(&mut book, buy);

    assert_eq!(result.status, OrderStatus::Filled);
    assert_eq!(result.filled_qty, dec(7));
    assert_eq!(result.trades.len(), 3);

    // Trades should be at ascending prices (best ask first)
    assert_eq!(result.trades[0].price, dec(100));
    assert_eq!(result.trades[0].qty, dec(2));
    assert_eq!(result.trades[1].price, dec(101));
    assert_eq!(result.trades[1].qty, dec(3));
    assert_eq!(result.trades[2].price, dec(102));
    assert_eq!(result.trades[2].qty, dec(2));

    // Level 100: empty
    let tick100 = price_to_tick(dec(100));
    assert_eq!(book.asks.get_qty(tick100), 0);
    assert!(book.ask_levels.get(&tick100).is_none());

    // Level 101: empty
    let tick101 = price_to_tick(dec(101));
    assert_eq!(book.asks.get_qty(tick101), 0);
    assert!(book.ask_levels.get(&tick101).is_none());

    // Level 102: 2 remaining
    let tick102 = price_to_tick(dec(102));
    assert_eq!(book.asks.get_qty(tick102), 2);
    let level = book
        .ask_levels
        .get(&tick102)
        .expect("ask level should exist at tick102");
    assert_eq!(level.orders[0].remaining, dec(2));
}

// ---------------------------------------------------------------------------
// Market sell order walks bids
// ---------------------------------------------------------------------------

#[test]
fn test_market_sell_walks_bids() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);

    book.insert(make_limit_order(Side::Buy, dec(102), dec(2), 1));
    book.insert(make_limit_order(Side::Buy, dec(101), dec(3), 2));
    book.insert(make_limit_order(Side::Buy, dec(100), dec(4), 3));

    let sell = make_market_order(Side::Sell, dec(6), 4);
    let result = match_order(&mut book, sell);

    assert_eq!(result.status, OrderStatus::Filled);
    assert_eq!(result.filled_qty, dec(6));
    assert_eq!(result.trades.len(), 3);

    // Trades should be at descending prices (best bid first)
    assert_eq!(result.trades[0].price, dec(102));
    assert_eq!(result.trades[0].qty, dec(2));
    assert_eq!(result.trades[1].price, dec(101));
    assert_eq!(result.trades[1].qty, dec(3));
    assert_eq!(result.trades[2].price, dec(100));
    assert_eq!(result.trades[2].qty, dec(1));

    // Level 102: empty, 101: empty, 100: 3 remaining
    let tick100 = price_to_tick(dec(100));
    assert_eq!(book.bids.get_qty(tick100), 3);
}

// ---------------------------------------------------------------------------
// Limit order price check — does not cross if price is too low/high
// ---------------------------------------------------------------------------

#[test]
fn test_limit_buy_price_too_low_does_not_match() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    book.insert(make_limit_order(Side::Sell, dec(100), dec(5), 1));

    let buy = make_limit_order(Side::Buy, dec(99), dec(5), 2);
    let buy_id = buy.id;
    let result = match_order(&mut book, buy);

    assert_eq!(result.status, OrderStatus::Open);
    assert_eq!(result.filled_qty, dec(0));
    assert_eq!(result.trades.len(), 0);

    // Buy should be resting at 99, sell still at 100
    assert_eq!(book.best_bid(), Some(dec(99)));
    assert_eq!(book.best_ask(), Some(dec(100)));
    assert!(book.order_index.contains_key(&buy_id));
}

#[test]
fn test_limit_sell_price_too_high_does_not_match() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    book.insert(make_limit_order(Side::Buy, dec(100), dec(5), 1));

    let sell = make_limit_order(Side::Sell, dec(101), dec(5), 2);
    let sell_id = sell.id;
    let result = match_order(&mut book, sell);

    assert_eq!(result.status, OrderStatus::Open);
    assert_eq!(result.filled_qty, dec(0));
    assert_eq!(result.trades.len(), 0);

    assert_eq!(book.best_bid(), Some(dec(100)));
    assert_eq!(book.best_ask(), Some(dec(101)));
    assert!(book.order_index.contains_key(&sell_id));
}

#[test]
fn test_limit_buy_crosses_spread() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    book.insert(make_limit_order(Side::Sell, dec(100), dec(5), 1));

    // Buy at 105 — should cross and fill at 100 (best ask)
    let buy = make_limit_order(Side::Buy, dec(105), dec(5), 2);
    let result = match_order(&mut book, buy);

    assert_eq!(result.status, OrderStatus::Filled);
    assert_eq!(result.filled_qty, dec(5));
    assert_eq!(result.trades[0].price, dec(100));
}

// ---------------------------------------------------------------------------
// IOC — immediate-or-cancel, remainder cancelled
// ---------------------------------------------------------------------------

#[test]
fn test_ioc_matches_then_cancels_remainder() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    book.insert(make_limit_order(Side::Sell, dec(100), dec(3), 1));

    let buy = make_ioc_order(Side::Buy, dec(100), dec(10), 2);
    let buy_id = buy.id;
    let result = match_order(&mut book, buy);

    assert_eq!(result.status, OrderStatus::PartiallyFilled);
    assert_eq!(result.filled_qty, dec(3));
    assert_eq!(result.trades.len(), 1);

    // Resting sell fully consumed
    let tick = price_to_tick(dec(100));
    assert!(book.ask_levels.get(&tick).is_none());

    // IOC remainder cancelled — NOT on the book
    assert!(!book.order_index.contains_key(&buy_id));
}

#[test]
fn test_ioc_no_match_cancels_entirely() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    // Empty book — no resting orders

    let buy = make_ioc_order(Side::Buy, dec(100), dec(10), 1);
    let buy_id = buy.id;
    let result = match_order(&mut book, buy);

    assert_eq!(result.status, OrderStatus::Cancelled);
    assert_eq!(result.filled_qty, dec(0));
    assert_eq!(result.trades.len(), 0);

    // Nothing on the book
    assert!(!book.order_index.contains_key(&buy_id));
    assert_eq!(book.best_bid(), None);
}

// ---------------------------------------------------------------------------
// FOK — fill-or-kill
// ---------------------------------------------------------------------------

#[test]
fn test_fok_full_fill_succeeds() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    book.insert(make_limit_order(Side::Sell, dec(100), dec(10), 1));

    let buy = make_fok_order(Side::Buy, dec(100), dec(5), 2);
    let result = match_order(&mut book, buy);

    assert_eq!(result.status, OrderStatus::Filled);
    assert_eq!(result.filled_qty, dec(5));
    assert_eq!(result.trades.len(), 1);
    assert_eq!(result.trades[0].qty, dec(5));

    // 5 remaining at sell level
    let tick = price_to_tick(dec(100));
    assert_eq!(book.asks.get_qty(tick), 5);
}

#[test]
fn test_fok_insufficient_liquidity_cancels_entirely() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    book.insert(make_limit_order(Side::Sell, dec(100), dec(3), 1));

    let buy = make_fok_order(Side::Buy, dec(100), dec(10), 2);
    let buy_id = buy.id;
    let result = match_order(&mut book, buy);

    assert_eq!(result.status, OrderStatus::Cancelled);
    assert_eq!(result.filled_qty, dec(0));
    assert_eq!(result.trades.len(), 0);

    // Book should be unchanged — no fills happened
    let tick = price_to_tick(dec(100));
    assert_eq!(book.asks.get_qty(tick), 3);
    assert!(!book.order_index.contains_key(&buy_id));
}

#[test]
fn test_fok_empty_book_cancels() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);

    let buy = make_fok_order(Side::Buy, dec(100), dec(5), 1);
    let buy_id = buy.id;
    let result = match_order(&mut book, buy);

    assert_eq!(result.status, OrderStatus::Cancelled);
    assert_eq!(result.filled_qty, dec(0));
    assert!(!book.order_index.contains_key(&buy_id));
}

// ---------------------------------------------------------------------------
// FOK — price constraint: must respect limit price in pre-check
// ---------------------------------------------------------------------------

#[test]
fn test_fok_limit_price_constraint_buy_insufficient_after_constraint() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    // Asks at 100, 101 (both cross limit 101), and 105 (does not cross limit 101)
    book.insert(make_limit_order(Side::Sell, dec(100), dec(3), 1));
    book.insert(make_limit_order(Side::Sell, dec(101), dec(3), 2));
    book.insert(make_limit_order(Side::Sell, dec(105), dec(10), 3));

    // FOK buy at limit 101 — only 100 and 101 cross (total 6), need 10 → cancel
    let buy = make_fok_order(Side::Buy, dec(101), dec(10), 4);
    let buy_id = buy.id;
    let result = match_order(&mut book, buy);

    assert_eq!(result.status, OrderStatus::Cancelled);
    assert_eq!(result.filled_qty, dec(0));
    assert_eq!(result.trades.len(), 0);

    // Book unchanged
    let tick100 = price_to_tick(dec(100));
    assert_eq!(book.asks.get_qty(tick100), 3);
    let tick101 = price_to_tick(dec(101));
    assert_eq!(book.asks.get_qty(tick101), 3);
    let tick105 = price_to_tick(dec(105));
    assert_eq!(book.asks.get_qty(tick105), 10);
    assert!(!book.order_index.contains_key(&buy_id));
}

#[test]
fn test_fok_limit_price_constraint_sell_insufficient_after_constraint() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    // Bids at 105, 103 (both cross limit 103), and 99 (does not cross limit 103)
    book.insert(make_limit_order(Side::Buy, dec(105), dec(3), 1));
    book.insert(make_limit_order(Side::Buy, dec(103), dec(3), 2));
    book.insert(make_limit_order(Side::Buy, dec(99), dec(10), 3));

    // FOK sell at limit 103 — only 105 and 103 cross (total 6), need 10 → cancel
    let sell = make_fok_order(Side::Sell, dec(103), dec(10), 4);
    let sell_id = sell.id;
    let result = match_order(&mut book, sell);

    assert_eq!(result.status, OrderStatus::Cancelled);
    assert_eq!(result.filled_qty, dec(0));
    assert_eq!(result.trades.len(), 0);

    // Book unchanged
    assert!(!book.order_index.contains_key(&sell_id));
}

#[test]
fn test_fok_buy_limit_price_crosses_all_levels_sufficient() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    book.insert(make_limit_order(Side::Sell, dec(100), dec(3), 1));
    book.insert(make_limit_order(Side::Sell, dec(101), dec(3), 2));

    // FOK buy at limit 101 — both levels cross, total 6 >= 5 → should fill
    let buy = make_fok_order(Side::Buy, dec(101), dec(5), 3);
    let result = match_order(&mut book, buy);

    assert_eq!(result.status, OrderStatus::Filled);
    assert_eq!(result.filled_qty, dec(5));
    assert_eq!(result.trades.len(), 2);
}

#[test]
fn test_fok_sell_limit_price_crosses_all_levels_sufficient() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    book.insert(make_limit_order(Side::Buy, dec(102), dec(3), 1));
    book.insert(make_limit_order(Side::Buy, dec(101), dec(3), 2));

    // FOK sell at limit 101 — both levels cross, total 6 >= 5 → should fill
    let sell = make_fok_order(Side::Sell, dec(101), dec(5), 3);
    let result = match_order(&mut book, sell);

    assert_eq!(result.status, OrderStatus::Filled);
    assert_eq!(result.filled_qty, dec(5));
    assert_eq!(result.trades.len(), 2);
}

// ---------------------------------------------------------------------------
// FOK with multiple levels
// ---------------------------------------------------------------------------

#[test]
fn test_fok_multiple_levels_sufficient_liquidity() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    book.insert(make_limit_order(Side::Sell, dec(100), dec(3), 1));
    book.insert(make_limit_order(Side::Sell, dec(101), dec(4), 2));
    book.insert(make_limit_order(Side::Sell, dec(102), dec(5), 3));

    let buy = make_fok_order(Side::Buy, dec(102), dec(10), 4);
    let result = match_order(&mut book, buy);

    assert_eq!(result.status, OrderStatus::Filled);
    assert_eq!(result.filled_qty, dec(10));
    assert_eq!(result.trades.len(), 3);

    assert_eq!(result.trades[0].qty, dec(3));
    assert_eq!(result.trades[1].qty, dec(4));
    assert_eq!(result.trades[2].qty, dec(3));

    // 2 remaining at 102
    let tick102 = price_to_tick(dec(102));
    assert_eq!(book.asks.get_qty(tick102), 2);
}

#[test]
fn test_fok_multiple_levels_insufficient_liquidity() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    book.insert(make_limit_order(Side::Sell, dec(100), dec(3), 1));
    book.insert(make_limit_order(Side::Sell, dec(101), dec(4), 2));

    let buy = make_fok_order(Side::Buy, dec(101), dec(10), 3);
    let buy_id = buy.id;
    let result = match_order(&mut book, buy);

    assert_eq!(result.status, OrderStatus::Cancelled);
    assert_eq!(result.filled_qty, dec(0));

    // Book unchanged
    let tick100 = price_to_tick(dec(100));
    assert_eq!(book.asks.get_qty(tick100), 3);
    let tick101 = price_to_tick(dec(101));
    assert_eq!(book.asks.get_qty(tick101), 4);
    assert!(!book.order_index.contains_key(&buy_id));
}

// ---------------------------------------------------------------------------
// Average fill price calculation
// ---------------------------------------------------------------------------

#[test]
fn test_average_fill_price_single_level() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    book.insert(make_limit_order(Side::Sell, dec(100), dec(5), 1));

    let buy = make_market_order(Side::Buy, dec(5), 2);
    let result = match_order(&mut book, buy);

    assert_eq!(result.avg_fill_price, Some(dec(100)));
}

#[test]
fn test_average_fill_price_multiple_levels() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    book.insert(make_limit_order(Side::Sell, dec(100), dec(2), 1));
    book.insert(make_limit_order(Side::Sell, dec(101), dec(3), 2));

    let buy = make_market_order(Side::Buy, dec(5), 3);
    let result = match_order(&mut book, buy);

    assert_eq!(result.filled_qty, dec(5));
    assert_eq!(result.trades.len(), 2);

    // avg = (2*100 + 3*101) / 5 = 503 / 5 = 100.6
    let expected_avg = dec(503) / dec(5);
    assert_eq!(result.avg_fill_price, Some(expected_avg));
}

// ---------------------------------------------------------------------------
// STP — CancelNewest (default: reject incoming)
// ---------------------------------------------------------------------------

#[test]
fn test_stp_cancel_newest_rejects_incoming() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    let resting = make_order_with_participant(Side::Sell, dec(100), dec(5), "trader_a", 1);
    let resting_id = resting.id;
    book.insert(resting);

    let incoming = make_stp_order(
        Side::Buy,
        dec(100),
        dec(5),
        "trader_a",
        STPMode::CancelNewest,
        2,
    );
    let incoming_id = incoming.id;
    let result = match_order(&mut book, incoming);

    assert_eq!(result.status, OrderStatus::Cancelled);
    assert_eq!(result.filled_qty, dec(0));
    assert_eq!(result.trades.len(), 0);

    // Resting order should still be on the book, unchanged
    let tick = price_to_tick(dec(100));
    assert_eq!(book.asks.get_qty(tick), 5);
    assert!(book.order_index.contains_key(&resting_id));
    assert!(!book.order_index.contains_key(&incoming_id));
}

// ---------------------------------------------------------------------------
// STP — CancelOldest (remove resting, continue matching)
// ---------------------------------------------------------------------------

#[test]
fn test_stp_cancel_oldest_removes_resting_matches_next() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);

    // Resting sell from trader_a
    let resting1 = make_order_with_participant(Side::Sell, dec(100), dec(5), "trader_a", 1);
    let resting1_id = resting1.id;
    book.insert(resting1);

    // Resting sell from trader_a at same price (this would be removed by STP)
    // Actually let's put a different participant's order:
    let resting2 = make_order_with_participant(Side::Sell, dec(100), dec(3), "trader_b", 2);
    let resting2_id = resting2.id;
    book.insert(resting2);

    // Incoming buy from trader_a with CancelOldest
    let incoming = make_stp_order(
        Side::Buy,
        dec(100),
        dec(5),
        "trader_a",
        STPMode::CancelOldest,
        3,
    );

    // ... wait, this setup has trader_a's order first, then trader_b's.
    // When trader_a comes in to buy, the STP check hits trader_a's own sell order.
    // CancelOldest removes resting1 (trader_a's sell), then the next order is trader_b's sell.
    // The incoming buy should match against trader_b's sell.

    let result = match_order(&mut book, incoming);

    // Should fill 3 from trader_b, remaining 2 of incoming goes on book
    assert_eq!(result.status, OrderStatus::PartiallyFilled);
    assert_eq!(result.filled_qty, dec(3));
    assert_eq!(result.trades.len(), 1);
    assert_eq!(result.trades[0].sell_participant_id, "trader_b");

    // resting1 (trader_a's sell) should be removed by STP
    assert!(!book.order_index.contains_key(&resting1_id));

    // resting2 (trader_b's sell) should be fully consumed
    assert!(!book.order_index.contains_key(&resting2_id));

    // 2 remaining of incoming should be resting as buy at 100
    let tick = price_to_tick(dec(100));
    assert_eq!(book.bids.get_qty(tick), 2);
}

// ---------------------------------------------------------------------------
// STP — Decrement
// ---------------------------------------------------------------------------

#[test]
fn test_stp_decrement_resting_larger_cancels_incoming() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    let resting = make_order_with_participant(Side::Sell, dec(100), dec(5), "trader_a", 1);
    let resting_id = resting.id;
    book.insert(resting);

    let incoming = make_stp_order(
        Side::Buy,
        dec(100),
        dec(3),
        "trader_a",
        STPMode::Decrement,
        2,
    );
    let incoming_id = incoming.id;
    let result = match_order(&mut book, incoming);

    // Incoming is fully consumed (resting 5 >= incoming 3)
    assert_eq!(result.status, OrderStatus::Cancelled);
    assert_eq!(result.filled_qty, dec(0));
    assert_eq!(result.trades.len(), 0);

    // Resting should be removed (CancelOldest behavior on the resting side)
    assert!(!book.order_index.contains_key(&resting_id));
    assert!(!book.order_index.contains_key(&incoming_id));
}

#[test]
fn test_stp_decrement_incoming_larger_continues_matching() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    let resting_stp = make_order_with_participant(Side::Sell, dec(100), dec(3), "trader_a", 1);
    let resting_stp_id = resting_stp.id;
    book.insert(resting_stp);

    let resting_other = make_order_with_participant(Side::Sell, dec(100), dec(5), "trader_b", 2);
    let resting_other_id = resting_other.id;
    book.insert(resting_other);

    let incoming = make_stp_order(
        Side::Buy,
        dec(100),
        dec(10),
        "trader_a",
        STPMode::Decrement,
        3,
    );
    let result = match_order(&mut book, incoming);

    // STP decrements incoming (10 - 3 = 7), removes resting_stp
    // Then matches 5 from resting_other, remaining 2 goes on book
    assert_eq!(result.status, OrderStatus::PartiallyFilled);
    assert_eq!(result.filled_qty, dec(5));
    assert_eq!(result.trades.len(), 1);
    assert_eq!(result.trades[0].sell_participant_id, "trader_b");
    assert_eq!(result.trades[0].qty, dec(5));

    // resting_stp should be removed
    assert!(!book.order_index.contains_key(&resting_stp_id));

    // resting_other should be fully consumed
    assert!(!book.order_index.contains_key(&resting_other_id));

    // 2 remaining on book
    let tick = price_to_tick(dec(100));
    assert_eq!(book.bids.get_qty(tick), 2);
}

// ---------------------------------------------------------------------------
// STP — None (allow self-trade)
// ---------------------------------------------------------------------------

#[test]
fn test_stp_none_allows_self_trade() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    let resting = make_order_with_participant(Side::Sell, dec(100), dec(5), "trader_a", 1);
    book.insert(resting);

    let incoming = make_stp_order(
        Side::Buy,
        dec(100),
        dec(3),
        "trader_a",
        STPMode::None,
        2,
    );
    let result = match_order(&mut book, incoming);

    // Should fill as normal (self-trade allowed)
    assert_eq!(result.status, OrderStatus::Filled);
    assert_eq!(result.filled_qty, dec(3));
    assert_eq!(result.trades.len(), 1);
    assert_eq!(result.trades[0].buy_participant_id, "trader_a");
    assert_eq!(result.trades[0].sell_participant_id, "trader_a");

    // 2 remaining of sell
    let tick = price_to_tick(dec(100));
    assert_eq!(book.asks.get_qty(tick), 2);
}

// ---------------------------------------------------------------------------
// Market order with empty book
// ---------------------------------------------------------------------------

#[test]
fn test_market_order_empty_book_cancels() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);

    let buy = make_market_order(Side::Buy, dec(5), 1);
    let buy_id = buy.id;
    let result = match_order(&mut book, buy);

    assert_eq!(result.status, OrderStatus::Cancelled);
    assert_eq!(result.filled_qty, dec(0));
    assert_eq!(result.trades.len(), 0);
    assert!(!book.order_index.contains_key(&buy_id));
}

// ---------------------------------------------------------------------------
// Market order exhausts contra side
// ---------------------------------------------------------------------------

#[test]
fn test_market_order_exhausts_contra_side() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    book.insert(make_limit_order(Side::Sell, dec(100), dec(2), 1));
    book.insert(make_limit_order(Side::Sell, dec(101), dec(1), 2));

    let buy = make_market_order(Side::Buy, dec(10), 3);
    let result = match_order(&mut book, buy);

    // Should fill all available (2+1=3) then cancel remainder
    assert_eq!(result.filled_qty, dec(3));
    assert_eq!(result.trades.len(), 2);

    // Book should be empty on sell side
    assert_eq!(book.best_ask(), None);

    // 7 remaining of buy should be cancelled (market order)
    assert_eq!(result.status, OrderStatus::PartiallyFilled);
}

// ---------------------------------------------------------------------------
// Multiple orders at same price level fill in FIFO order
// ---------------------------------------------------------------------------

#[test]
fn test_multiple_orders_same_price_fifo_fill() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    let o1 = make_limit_order(Side::Sell, dec(100), dec(3), 1);
    let o2 = make_limit_order(Side::Sell, dec(100), dec(5), 2);
    let o3 = make_limit_order(Side::Sell, dec(100), dec(2), 3);
    let o1_id = o1.id;
    let o2_id = o2.id;
    let o3_id = o3.id;

    book.insert(o1);
    book.insert(o2);
    book.insert(o3);

    let buy = make_market_order(Side::Buy, dec(6), 4);
    let result = match_order(&mut book, buy);

    assert_eq!(result.filled_qty, dec(6));

    // First trade: o1 (3 qty)
    assert_eq!(result.trades[0].qty, dec(3));
    assert_eq!(result.trades[0].sell_order_id, o1_id);

    // Second trade: o2 (3 of 5 qty)
    assert_eq!(result.trades[1].qty, dec(3));
    assert_eq!(result.trades[1].sell_order_id, o2_id);

    // o2 should have 2 remaining, o3 should have 2 remaining
    let tick = price_to_tick(dec(100));
    let level = book
        .ask_levels
        .get(&tick)
        .expect("ask level should exist at tick");
    assert_eq!(level.orders.len(), 2);
    assert_eq!(level.orders[0].id, o2_id);
    assert_eq!(level.orders[0].remaining, dec(2));
    assert_eq!(level.orders[1].id, o3_id);
    assert_eq!(level.orders[1].remaining, dec(2));
}

// ---------------------------------------------------------------------------
// Best bid/ask update after fills
// ---------------------------------------------------------------------------

#[test]
fn test_best_ask_updates_after_level_emptied_by_fills() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    book.insert(make_limit_order(Side::Sell, dec(100), dec(1), 1));
    book.insert(make_limit_order(Side::Sell, dec(101), dec(2), 2));
    assert_eq!(book.best_ask(), Some(dec(100)));

    let buy = make_market_order(Side::Buy, dec(1), 3);
    match_order(&mut book, buy);

    assert_eq!(book.best_ask(), Some(dec(101)));

    let buy2 = make_market_order(Side::Buy, dec(2), 4);
    match_order(&mut book, buy2);

    assert_eq!(book.best_ask(), None);
}

#[test]
fn test_best_bid_updates_after_level_emptied_by_fills() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    book.insert(make_limit_order(Side::Buy, dec(102), dec(1), 1));
    book.insert(make_limit_order(Side::Buy, dec(101), dec(2), 2));
    assert_eq!(book.best_bid(), Some(dec(102)));

    let sell = make_market_order(Side::Sell, dec(1), 3);
    match_order(&mut book, sell);

    assert_eq!(book.best_bid(), Some(dec(101)));

    let sell2 = make_market_order(Side::Sell, dec(2), 4);
    match_order(&mut book, sell2);

    assert_eq!(book.best_bid(), None);
}

// ---------------------------------------------------------------------------
// Trade contains correct buy/sell participant IDs
// ---------------------------------------------------------------------------

#[test]
fn test_trade_participant_ids() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    let sell = make_order_with_participant(Side::Sell, dec(100), dec(5), "seller_a", 1);
    book.insert(sell);

    let buy = make_order_with_participant(Side::Buy, dec(100), dec(3), "buyer_b", 2);
    let result = match_order(&mut book, buy);

    assert_eq!(result.trades.len(), 1);
    assert_eq!(result.trades[0].buy_participant_id, "buyer_b");
    assert_eq!(result.trades[0].sell_participant_id, "seller_a");
}

// ---------------------------------------------------------------------------
// Fill-or-kill cross-side
// ---------------------------------------------------------------------------

#[test]
fn test_fok_sell_side() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    book.insert(make_limit_order(Side::Buy, dec(100), dec(5), 1));

    // FOK sell at 100 — sufficient liquidity
    let sell = make_fok_order(Side::Sell, dec(100), dec(5), 2);
    let result = match_order(&mut book, sell);

    assert_eq!(result.status, OrderStatus::Filled);
    assert_eq!(result.filled_qty, dec(5));
    assert_eq!(result.trades.len(), 1);
}

#[test]
fn test_fok_sell_insufficient_cancels() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    book.insert(make_limit_order(Side::Buy, dec(100), dec(3), 1));

    let sell = make_fok_order(Side::Sell, dec(100), dec(10), 2);
    let sell_id = sell.id;
    let result = match_order(&mut book, sell);

    assert_eq!(result.status, OrderStatus::Cancelled);
    assert_eq!(result.filled_qty, dec(0));

    // Book unchanged
    let tick = price_to_tick(dec(100));
    assert_eq!(book.bids.get_qty(tick), 3);
    assert!(!book.order_index.contains_key(&sell_id));
}

// ---------------------------------------------------------------------------
// Input validation — reject invalid orders
// ---------------------------------------------------------------------------

#[test]
fn test_reject_zero_quantity() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    let order = make_limit_order(Side::Buy, dec(100), dec(0), 1);
    let order_id = order.id;
    let result = match_order(&mut book, order);

    assert_eq!(result.status, OrderStatus::Rejected);
    assert_eq!(result.filled_qty, dec(0));
    assert_eq!(result.trades.len(), 0);
    assert!(!book.order_index.contains_key(&order_id));
}

#[test]
fn test_reject_negative_quantity() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    let id = Uuid::new_v4();
    let order = Order::new(
        id,
        1,
        "trader_a".into(),
        Side::Buy,
        OrderType::Limit,
        Some(dec(100)),
        Decimal::new(-1, 0),
        1,
        STPMode::None,
    );
    let result = match_order(&mut book, order);

    assert_eq!(result.status, OrderStatus::Rejected);
    assert_eq!(result.filled_qty, dec(0));
    assert!(!book.order_index.contains_key(&id));
}

#[test]
fn test_reject_zero_price_limit_order() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    let order = make_limit_order(Side::Buy, dec(0), dec(5), 1);
    let order_id = order.id;
    let result = match_order(&mut book, order);

    assert_eq!(result.status, OrderStatus::Rejected);
    assert_eq!(result.filled_qty, dec(0));
    assert!(!book.order_index.contains_key(&order_id));
}

#[test]
fn test_reject_negative_price_limit_order() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    let id = Uuid::new_v4();
    let order = Order::new(
        id,
        1,
        "trader_a".into(),
        Side::Buy,
        OrderType::Limit,
        Some(Decimal::new(-100, 0)),
        dec(5),
        1,
        STPMode::None,
    );
    let result = match_order(&mut book, order);

    assert_eq!(result.status, OrderStatus::Rejected);
    assert_eq!(result.filled_qty, dec(0));
    assert!(!book.order_index.contains_key(&id));
}

#[test]
fn test_reject_price_exceeding_max_ticks() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    // price = TEST_MAX_TICKS * 0.01 = 500.00, which produces tick == TEST_MAX_TICKS
    // which is >= max_ticks, so it should be rejected
    let max_price = Decimal::new(TEST_MAX_TICKS as i64, 2); // = 500.00
    let order = make_limit_order(Side::Buy, max_price, dec(5), 1);
    let order_id = order.id;
    let result = match_order(&mut book, order);

    assert_eq!(result.status, OrderStatus::Rejected);
    assert_eq!(result.filled_qty, dec(0));
    assert!(!book.order_index.contains_key(&order_id));
}

#[test]
fn test_accept_max_tick_price() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    // price just under the max tick boundary
    let max_price = Decimal::new((TEST_MAX_TICKS - 1) as i64, 2);
    let order = make_limit_order(Side::Buy, max_price, dec(5), 1);
    let order_id = order.id;
    let result = match_order(&mut book, order);

    // No contra liquidity, so should rest on book (Open), NOT be rejected
    assert_eq!(result.status, OrderStatus::Open);
    assert!(book.order_index.contains_key(&order_id));
}

#[test]
fn test_market_order_with_zero_price_accepted() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    book.insert(make_limit_order(Side::Sell, dec(100), dec(5), 1));

    let buy = make_market_order(Side::Buy, dec(3), 2);
    let result = match_order(&mut book, buy);

    // Market orders have no price, so validation should allow them
    assert_eq!(result.status, OrderStatus::Filled);
    assert_eq!(result.filled_qty, dec(3));
}

// ---------------------------------------------------------------------------
// Decimal tick/qty round trip
// ---------------------------------------------------------------------------

#[test]
fn test_decimal_price_tick_conversion() {
    // decimal_to_tick_qty converts Decimal to u64 for tick array
    let qty = decimal_to_tick_qty(dec(5));
    assert_eq!(qty, 5);

    let qty2 = decimal_to_tick_qty(dec(0));
    assert_eq!(qty2, 0);
}
