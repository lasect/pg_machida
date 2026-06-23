use pg_machida::types::*;
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Side — string conversions
// ---------------------------------------------------------------------------

#[test]
fn side_from_str_buy() {
    assert_eq!(Side::try_from("buy").expect("valid side"), Side::Buy);
    assert_eq!(Side::try_from("BUY").expect("valid side"), Side::Buy);
    assert_eq!(Side::from_str("buy").expect("valid side"), Side::Buy);
}

#[test]
fn side_from_str_sell() {
    assert_eq!(Side::try_from("sell").expect("valid side"), Side::Sell);
    assert_eq!(Side::try_from("SELL").expect("valid side"), Side::Sell);
}

#[test]
fn side_to_str() {
    assert_eq!(<&str>::from(Side::Buy), "buy");
    assert_eq!(<&str>::from(Side::Sell), "sell");
}

#[test]
fn side_display() {
    assert_eq!(format!("{}", Side::Buy), "buy");
    assert_eq!(format!("{}", Side::Sell), "sell");
}

#[test]
fn side_from_str_invalid() {
    assert!(Side::try_from("foo").is_err());
}

// ---------------------------------------------------------------------------
// OrderType — string conversions
// ---------------------------------------------------------------------------

#[test]
fn order_type_from_str_all() {
    assert_eq!(OrderType::try_from("limit").expect("valid order_type"), OrderType::Limit);
    assert_eq!(OrderType::try_from("LIMIT").expect("valid order_type"), OrderType::Limit);
    assert_eq!(OrderType::try_from("market").expect("valid order_type"), OrderType::Market);
    assert_eq!(OrderType::try_from("ioc").expect("valid order_type"), OrderType::IOC);
    assert_eq!(OrderType::try_from("IOC").expect("valid order_type"), OrderType::IOC);
    assert_eq!(OrderType::try_from("fok").expect("valid order_type"), OrderType::FOK);
}

#[test]
fn order_type_to_str() {
    assert_eq!(<&str>::from(OrderType::Limit), "limit");
    assert_eq!(<&str>::from(OrderType::Market), "market");
    assert_eq!(<&str>::from(OrderType::IOC), "ioc");
    assert_eq!(<&str>::from(OrderType::FOK), "fok");
}

#[test]
fn order_type_display() {
    assert_eq!(format!("{}", OrderType::Limit), "limit");
    assert_eq!(format!("{}", OrderType::FOK), "fok");
}

#[test]
fn order_type_from_str_invalid() {
    assert!(OrderType::try_from("gtc").is_err());
}

// ---------------------------------------------------------------------------
// OrderStatus — string conversions
// ---------------------------------------------------------------------------

#[test]
fn order_status_from_str_all() {
    assert_eq!(OrderStatus::try_from("open").expect("valid order_status"), OrderStatus::Open);
    assert_eq!(OrderStatus::try_from("OPEN").expect("valid order_status"), OrderStatus::Open);
    assert_eq!(OrderStatus::try_from("partially_filled").expect("valid order_status"), OrderStatus::PartiallyFilled);
    assert_eq!(OrderStatus::try_from("filled").expect("valid order_status"), OrderStatus::Filled);
    assert_eq!(OrderStatus::try_from("cancelled").expect("valid order_status"), OrderStatus::Cancelled);
    assert_eq!(OrderStatus::try_from("rejected").expect("valid order_status"), OrderStatus::Rejected);
}

#[test]
fn order_status_to_str() {
    assert_eq!(<&str>::from(OrderStatus::Open), "open");
    assert_eq!(<&str>::from(OrderStatus::PartiallyFilled), "partially_filled");
    assert_eq!(<&str>::from(OrderStatus::Filled), "filled");
    assert_eq!(<&str>::from(OrderStatus::Cancelled), "cancelled");
    assert_eq!(<&str>::from(OrderStatus::Rejected), "rejected");
}

#[test]
fn order_status_display() {
    assert_eq!(format!("{}", OrderStatus::Cancelled), "cancelled");
    assert_eq!(format!("{}", OrderStatus::Rejected), "rejected");
}

#[test]
fn order_status_from_str_invalid() {
    assert!(OrderStatus::try_from("unknown").is_err());
}

// ---------------------------------------------------------------------------
// STPMode — string conversions
// ---------------------------------------------------------------------------

#[test]
fn stp_mode_from_str_all() {
    assert_eq!(STPMode::try_from("cancel_newest").expect("valid stp_mode"), STPMode::CancelNewest);
    assert_eq!(STPMode::try_from("CANCEL_NEWEST").expect("valid stp_mode"), STPMode::CancelNewest);
    assert_eq!(STPMode::try_from("cancel_oldest").expect("valid stp_mode"), STPMode::CancelOldest);
    assert_eq!(STPMode::try_from("decrement").expect("valid stp_mode"), STPMode::Decrement);
    assert_eq!(STPMode::try_from("none").expect("valid stp_mode"), STPMode::None);
}

#[test]
fn stp_mode_to_str() {
    assert_eq!(<&str>::from(STPMode::CancelNewest), "cancel_newest");
    assert_eq!(<&str>::from(STPMode::CancelOldest), "cancel_oldest");
    assert_eq!(<&str>::from(STPMode::Decrement), "decrement");
    assert_eq!(<&str>::from(STPMode::None), "none");
}

#[test]
fn stp_mode_display() {
    assert_eq!(format!("{}", STPMode::CancelNewest), "cancel_newest");
    assert_eq!(format!("{}", STPMode::None), "none");
}

#[test]
fn stp_mode_from_str_invalid() {
    assert!(STPMode::try_from("cancel_both").is_err());
}

// ---------------------------------------------------------------------------
// Order::new() defaults
// ---------------------------------------------------------------------------

#[test]
fn order_new_sets_defaults() {
    let order = Order::new(
        Uuid::new_v4(),
        1,
        "trader_1".into(),
        Side::Buy,
        OrderType::Limit,
        Some(Decimal::new(10000, 0)),
        Decimal::new(5, 0),
        1_000_000,
        STPMode::CancelNewest,
    );
    assert_eq!(order.status, OrderStatus::Open);
    assert_eq!(order.remaining, Decimal::new(5, 0));
    assert_eq!(order.qty, Decimal::new(5, 0));
}

// ---------------------------------------------------------------------------
// PlaceOrderResult helpers
// ---------------------------------------------------------------------------

#[test]
fn place_order_result_cancelled() {
    let id = Uuid::new_v4();
    let result = PlaceOrderResult::cancelled(id);
    assert_eq!(result.order_id, id);
    assert_eq!(result.status, OrderStatus::Cancelled);
    assert_eq!(result.filled_qty, Decimal::ZERO);
    assert!(result.avg_fill_price.is_none());
    assert!(result.trades.is_empty());
}

#[test]
fn place_order_result_rejected() {
    let id = Uuid::new_v4();
    let result = PlaceOrderResult::rejected(id);
    assert_eq!(result.order_id, id);
    assert_eq!(result.status, OrderStatus::Rejected);
    assert_eq!(result.filled_qty, Decimal::ZERO);
    assert!(result.avg_fill_price.is_none());
    assert!(result.trades.is_empty());
}
