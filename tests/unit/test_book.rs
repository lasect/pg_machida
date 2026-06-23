use pg_machida::book::*;
use pg_machida::types::*;
use rust_decimal::Decimal;
use uuid::Uuid;

const TEST_MAX_TICKS: usize = 50000;

fn make_order(side: Side, price: Option<Decimal>, qty: Decimal) -> Order {
    Order::new(
        Uuid::new_v4(),
        1,
        "test".into(),
        side,
        OrderType::Limit,
        price,
        qty,
        1_000_000,
        STPMode::None,
    )
}

fn order_with_id(id: Uuid, side: Side, price: Option<Decimal>, qty: Decimal, ts: u64) -> Order {
    Order::new(
        id,
        1,
        "test".into(),
        side,
        OrderType::Limit,
        price,
        qty,
        ts,
        STPMode::None,
    )
}

// ---------------------------------------------------------------------------
// price_to_tick / tick_to_price round trip
// ---------------------------------------------------------------------------

#[test]
fn test_price_tick_round_trip() {
    for price in [Decimal::new(100, 0), Decimal::new(1, 2), Decimal::new(99999, 0)] {
        let tick = price_to_tick(price);
        let back = tick_to_decimal(tick);
        assert_eq!(back, price, "round trip failed for {}", price);
    }
}

// ---------------------------------------------------------------------------
// TickArray — basic operations
// ---------------------------------------------------------------------------

#[test]
fn test_tick_array_add_and_get() {
    let arr = TickArray::new(1000, 0);
    arr.add_qty(100, 5);
    assert_eq!(arr.get_qty(100), 5);
    arr.add_qty(100, 3);
    assert_eq!(arr.get_qty(100), 8);
    arr.sub_qty(100, 2);
    assert_eq!(arr.get_qty(100), 6);
}

#[test]
fn test_tick_array_best_tick_get_set() {
    let arr = TickArray::new(1000, 0);
    arr.set_best_tick(42);
    assert_eq!(arr.get_best_tick(), 42);
}

#[test]
fn test_tick_array_update_best_bid() {
    let arr = TickArray::new(1000, 0);
    arr.add_qty(90, 1);
    arr.add_qty(95, 1);
    arr.set_best_tick(100); // start search from 100

    arr.update_best_bid();
    // Should find 95 (highest tick with qty > 0)
    assert_eq!(arr.get_best_tick(), 95);
}

#[test]
fn test_tick_array_update_best_bid_empty() {
    let arr = TickArray::new(1000, 0);
    arr.set_best_tick(100);
    arr.update_best_bid();
    assert_eq!(arr.get_best_tick(), 0);
    assert_eq!(arr.get_qty(0), 0);
}

#[test]
fn test_tick_array_update_best_ask() {
    let arr = TickArray::new(1000, 1000 - 1);
    arr.add_qty(105, 1);
    arr.add_qty(110, 1);
    arr.set_best_tick(100); // start search from 100

    arr.update_best_ask();
    // Should find 105 (lowest tick with qty > 0)
    assert_eq!(arr.get_best_tick(), 105);
}

#[test]
fn test_tick_array_update_best_ask_empty() {
    let arr = TickArray::new(1000, 1000 - 1);
    arr.set_best_tick(100);
    arr.update_best_ask();
    assert_eq!(arr.get_best_tick(), 999);
    assert_eq!(arr.get_qty(999), 0);
}

// ---------------------------------------------------------------------------
// OrderBook — insert + cancel round trip
// ---------------------------------------------------------------------------

#[test]
fn test_insert_and_cancel_round_trip_bid() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    let order = make_order(Side::Buy, Some(Decimal::new(99, 0)), Decimal::new(1, 0));
    let id = order.id;

    book.insert(order);

    let tick = price_to_tick(Decimal::new(99, 0));
    assert!(book.bids.get_qty(tick) > 0);
    assert_eq!(
        book.bid_levels
            .get(&tick)
            .expect("bid level should exist after insert")
            .orders
            .len(),
        1
    );
    assert_eq!(book.order_index.len(), 1);

    let cancelled = book.cancel(id);
    assert!(cancelled.is_some());
    assert_eq!(book.bids.get_qty(tick), 0);
    assert!(book.bid_levels.get(&tick).is_none());
    assert!(book.order_index.is_empty());
}

#[test]
fn test_insert_and_cancel_round_trip_ask() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    let order = make_order(Side::Sell, Some(Decimal::new(101, 0)), Decimal::new(2, 0));
    let id = order.id;

    book.insert(order);

    let tick = price_to_tick(Decimal::new(101, 0));
    assert!(book.asks.get_qty(tick) > 0);
    assert_eq!(
        book.ask_levels
            .get(&tick)
            .expect("ask level should exist after insert")
            .orders
            .len(),
        1
    );
    assert_eq!(book.order_index.len(), 1);

    let cancelled = book.cancel(id);
    assert!(cancelled.is_some());
    assert_eq!(book.asks.get_qty(tick), 0);
    assert!(book.ask_levels.get(&tick).is_none());
    assert!(book.order_index.is_empty());
}

// ---------------------------------------------------------------------------
// OrderBook — best_bid / best_ask
// ---------------------------------------------------------------------------

#[test]
fn test_best_bid_after_insert() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    book.insert(make_order(Side::Buy, Some(Decimal::new(99, 0)), Decimal::new(1, 0)));
    assert_eq!(book.best_bid(), Some(Decimal::new(99, 0)));
}

#[test]
fn test_best_ask_after_insert() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    book.insert(make_order(Side::Sell, Some(Decimal::new(101, 0)), Decimal::new(1, 0)));
    assert_eq!(book.best_ask(), Some(Decimal::new(101, 0)));
}

#[test]
fn test_best_bid_after_cancel() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    let order = make_order(Side::Buy, Some(Decimal::new(99, 0)), Decimal::new(1, 0));
    let id = order.id;
    book.insert(order);
    assert_eq!(book.best_bid(), Some(Decimal::new(99, 0)));
    book.cancel(id);
    assert_eq!(book.best_bid(), None);
}

#[test]
fn test_best_ask_after_cancel() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    let order = make_order(Side::Sell, Some(Decimal::new(101, 0)), Decimal::new(1, 0));
    let id = order.id;
    book.insert(order);
    assert_eq!(book.best_ask(), Some(Decimal::new(101, 0)));
    book.cancel(id);
    assert_eq!(book.best_ask(), None);
}

#[test]
fn test_best_bid_highest_price_multiple_levels() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    book.insert(make_order(Side::Buy, Some(Decimal::new(99, 0)), Decimal::new(1, 0)));
    book.insert(make_order(Side::Buy, Some(Decimal::new(100, 0)), Decimal::new(1, 0)));
    book.insert(make_order(Side::Buy, Some(Decimal::new(98, 0)), Decimal::new(1, 0)));
    assert_eq!(book.best_bid(), Some(Decimal::new(100, 0)));
}

#[test]
fn test_best_ask_lowest_price_multiple_levels() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    book.insert(make_order(Side::Sell, Some(Decimal::new(101, 0)), Decimal::new(1, 0)));
    book.insert(make_order(Side::Sell, Some(Decimal::new(100, 0)), Decimal::new(1, 0)));
    book.insert(make_order(Side::Sell, Some(Decimal::new(102, 0)), Decimal::new(1, 0)));
    assert_eq!(book.best_ask(), Some(Decimal::new(100, 0)));
}

#[test]
fn test_empty_book_best_bid_ask_none() {
    let book = OrderBook::new(TEST_MAX_TICKS);
    assert_eq!(book.best_bid(), None);
    assert_eq!(book.best_ask(), None);
}

#[test]
fn test_cancel_nonexistent_order() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    assert!(book.cancel(Uuid::new_v4()).is_none());
}

// ---------------------------------------------------------------------------
// OrderBook — best_bid/ask after filling top level
// ---------------------------------------------------------------------------

#[test]
fn test_best_bid_updates_after_top_level_emptied() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();
    book.insert(order_with_id(id1, Side::Buy, Some(Decimal::new(100, 0)), Decimal::new(1, 0), 1));
    book.insert(order_with_id(id2, Side::Buy, Some(Decimal::new(99, 0)), Decimal::new(1, 0), 2));
    assert_eq!(book.best_bid(), Some(Decimal::new(100, 0)));

    book.cancel(id1);
    assert_eq!(book.best_bid(), Some(Decimal::new(99, 0)));

    book.cancel(id2);
    assert_eq!(book.best_bid(), None);
}

#[test]
fn test_best_ask_updates_after_top_level_emptied() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();
    book.insert(order_with_id(id1, Side::Sell, Some(Decimal::new(100, 0)), Decimal::new(1, 0), 1));
    book.insert(order_with_id(id2, Side::Sell, Some(Decimal::new(101, 0)), Decimal::new(1, 0), 2));
    assert_eq!(book.best_ask(), Some(Decimal::new(100, 0)));

    book.cancel(id1);
    assert_eq!(book.best_ask(), Some(Decimal::new(101, 0)));

    book.cancel(id2);
    assert_eq!(book.best_ask(), None);
}

// ---------------------------------------------------------------------------
// OrderBook — depth
// ---------------------------------------------------------------------------

#[test]
fn test_depth_bids() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    book.insert(make_order(Side::Buy, Some(Decimal::new(99, 0)), Decimal::new(2, 0)));
    book.insert(make_order(Side::Buy, Some(Decimal::new(99, 0)), Decimal::new(1, 0)));
    book.insert(make_order(Side::Buy, Some(Decimal::new(98, 0)), Decimal::new(5, 0)));
    book.insert(make_order(Side::Buy, Some(Decimal::new(97, 0)), Decimal::new(3, 0)));

    let depth = book.depth(Side::Buy, 2);
    assert_eq!(depth.len(), 2);
    assert_eq!(depth[0].price, Decimal::new(99, 0));
    assert_eq!(depth[0].qty, Decimal::new(3, 0));
    assert_eq!(depth[0].order_count, 2);
    assert_eq!(depth[1].price, Decimal::new(98, 0));
    assert_eq!(depth[1].qty, Decimal::new(5, 0));
    assert_eq!(depth[1].order_count, 1);
}

#[test]
fn test_depth_asks() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    book.insert(make_order(Side::Sell, Some(Decimal::new(101, 0)), Decimal::new(1, 0)));
    book.insert(make_order(Side::Sell, Some(Decimal::new(101, 0)), Decimal::new(3, 0)));
    book.insert(make_order(Side::Sell, Some(Decimal::new(102, 0)), Decimal::new(2, 0)));

    let depth = book.depth(Side::Sell, 2);
    assert_eq!(depth.len(), 2);
    assert_eq!(depth[0].price, Decimal::new(101, 0));
    assert_eq!(depth[0].qty, Decimal::new(4, 0));
    assert_eq!(depth[0].order_count, 2);
    assert_eq!(depth[1].price, Decimal::new(102, 0));
    assert_eq!(depth[1].qty, Decimal::new(2, 0));
    assert_eq!(depth[1].order_count, 1);
}

#[test]
fn test_depth_empty_book() {
    let book = OrderBook::new(TEST_MAX_TICKS);
    assert_eq!(book.depth(Side::Buy, 5).len(), 0);
    assert_eq!(book.depth(Side::Sell, 5).len(), 0);
}

#[test]
fn test_depth_asks_after_cancel_top_level() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    let id1 = Uuid::new_v4();
    book.insert(order_with_id(id1, Side::Sell, Some(Decimal::new(100, 0)), Decimal::new(1, 0), 1));
    book.insert(make_order(Side::Sell, Some(Decimal::new(101, 0)), Decimal::new(2, 0)));

    let depth_before = book.depth(Side::Sell, 2);
    assert_eq!(depth_before[0].price, Decimal::new(100, 0));

    book.cancel(id1);

    let depth_after = book.depth(Side::Sell, 2);
    assert_eq!(depth_after[0].price, Decimal::new(101, 0));
    assert_eq!(depth_after.len(), 1);
}

// ---------------------------------------------------------------------------
// OrderBook — multiple orders at same tick (FIFO order in deque)
// ---------------------------------------------------------------------------

#[test]
fn test_multiple_orders_same_tick_fifo_order() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    book.insert(make_order(Side::Buy, Some(Decimal::new(99, 0)), Decimal::new(1, 0)));
    book.insert(make_order(Side::Buy, Some(Decimal::new(99, 0)), Decimal::new(2, 0)));
    book.insert(make_order(Side::Buy, Some(Decimal::new(99, 0)), Decimal::new(3, 0)));

    let tick = price_to_tick(Decimal::new(99, 0));
    let level = book
        .bid_levels
        .get(&tick)
        .expect("bid level should exist for FIFO test");
    assert_eq!(level.orders.len(), 3);
    assert_eq!(level.total_qty, Decimal::new(6, 0));
    // Orders should be in insertion order (FIFO)
    assert_eq!(level.orders[0].qty, Decimal::new(1, 0));
    assert_eq!(level.orders[1].qty, Decimal::new(2, 0));
    assert_eq!(level.orders[2].qty, Decimal::new(3, 0));
}

// ---------------------------------------------------------------------------
// OrderBook — market orders are not inserted
// ---------------------------------------------------------------------------

#[test]
fn test_market_order_not_inserted() {
    let mut book = OrderBook::new(TEST_MAX_TICKS);
    let order = make_order(Side::Buy, None, Decimal::new(1, 0));
    let id = order.id;
    book.insert(order);
    assert!(book.order_index.is_empty());
    assert!(book.cancel(id).is_none());
}
