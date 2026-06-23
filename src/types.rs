use rust_decimal::Decimal;
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

// Enums

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderType {
    Limit,
    Market,
    IOC,
    FOK,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderStatus {
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum STPMode {
    CancelNewest,
    CancelOldest,
    Decrement,
    None,
}

// Core structs

#[derive(Clone, Debug)]
pub struct Order {
    pub id: Uuid,
    pub instrument_id: u64,
    pub participant_id: String,
    pub side: Side,
    pub order_type: OrderType,
    pub price: Option<Decimal>,
    pub qty: Decimal,
    pub remaining: Decimal,
    pub status: OrderStatus,
    pub ts: u64,
    pub stp_mode: STPMode,
}

#[derive(Clone, Debug)]
pub struct Trade {
    pub id: Uuid,
    pub instrument_id: u64,
    pub buy_order_id: Uuid,
    pub sell_order_id: Uuid,
    pub buy_participant_id: String,
    pub sell_participant_id: String,
    pub price: Decimal,
    pub qty: Decimal,
    pub ts: u64,
}

impl Trade {
    /// Compute a deterministic, idempotent trade ID from the trade contents.
    /// Uses UUIDv5 (SHA-1) so re-inserting the same trade is safe with
    /// `ON CONFLICT DO NOTHING`.
    pub fn compute_id(
        instrument_id: u64,
        buy_order_id: Uuid,
        sell_order_id: Uuid,
        price: Decimal,
        qty: Decimal,
        fill_seq: u64,
    ) -> Uuid {
        let namespace = Uuid::parse_str("e7d8a1b4-3f95-4a2c-8e1d-c6b5f0a9d3e2")
            .expect("hard-coded namespace must parse");
        let name = format!(
            "{}:{}:{}:{}:{}:{}",
            instrument_id, buy_order_id, sell_order_id, price, qty, fill_seq
        );
        Uuid::new_v5(&namespace, name.as_bytes())
    }
}

#[derive(Clone, Debug)]
pub struct PlaceOrderResult {
    pub order_id: Uuid,
    pub status: OrderStatus,
    pub filled_qty: Decimal,
    pub avg_fill_price: Option<Decimal>,
    pub trades: Vec<Trade>,
}

#[derive(Clone, Debug)]
pub struct BookLevel {
    pub price: Decimal,
    pub qty: Decimal,
    pub order_count: u32,
}

// Extended types

#[derive(Clone, Debug)]
pub struct IcebergOrder {
    pub base: Order,
    pub hidden_qty: Decimal,
    pub slice_qty: Decimal,
}

#[derive(Clone, Debug)]
pub struct CircuitBreaker {
    pub reference_price: Decimal,
    pub halt_pct: Decimal,
    pub cooldown_secs: u64,
}

#[derive(Clone, Debug)]
pub struct RiskLimits {
    pub max_order_qty: Decimal,
    pub max_order_notional: Decimal,
    pub max_open_orders: u32,
    pub daily_notional_limit: Decimal,
}


// PlaceOrderResult helpers


impl PlaceOrderResult {
    pub fn cancelled(order_id: Uuid) -> Self {
        PlaceOrderResult {
            order_id,
            status: OrderStatus::Cancelled,
            filled_qty: Decimal::ZERO,
            avg_fill_price: None,
            trades: Vec::new(),
        }
    }

    pub fn rejected(order_id: Uuid) -> Self {
        PlaceOrderResult {
            order_id,
            status: OrderStatus::Rejected,
            filled_qty: Decimal::ZERO,
            avg_fill_price: None,
            trades: Vec::new(),
        }
    }
}


// Order helpers


impl Order {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: Uuid,
        instrument_id: u64,
        participant_id: String,
        side: Side,
        order_type: OrderType,
        price: Option<Decimal>,
        qty: Decimal,
        ts: u64,
        stp_mode: STPMode,
    ) -> Self {
        Order {
            id,
            instrument_id,
            participant_id,
            side,
            order_type,
            price,
            qty,
            remaining: qty,
            status: OrderStatus::Open,
            ts,
            stp_mode,
        }
    }
}


// String conversions — Side


impl From<Side> for &str {
    fn from(side: Side) -> &'static str {
        match side {
            Side::Buy => "buy",
            Side::Sell => "sell",
        }
    }
}

impl TryFrom<&str> for Side {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "buy" => Ok(Side::Buy),
            "sell" => Ok(Side::Sell),
            other => Err(format!("invalid side: {}", other)),
        }
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", <&str>::from(*self))
    }
}


// String conversions — OrderType


impl From<OrderType> for &str {
    fn from(ot: OrderType) -> &'static str {
        match ot {
            OrderType::Limit => "limit",
            OrderType::Market => "market",
            OrderType::IOC => "ioc",
            OrderType::FOK => "fok",
        }
    }
}

impl TryFrom<&str> for OrderType {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "limit" => Ok(OrderType::Limit),
            "market" => Ok(OrderType::Market),
            "ioc" => Ok(OrderType::IOC),
            "fok" => Ok(OrderType::FOK),
            other => Err(format!("invalid order type: {}", other)),
        }
    }
}

impl fmt::Display for OrderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", <&str>::from(*self))
    }
}


// String conversions — OrderStatus


impl From<OrderStatus> for &str {
    fn from(status: OrderStatus) -> &'static str {
        match status {
            OrderStatus::Open => "open",
            OrderStatus::PartiallyFilled => "partially_filled",
            OrderStatus::Filled => "filled",
            OrderStatus::Cancelled => "cancelled",
            OrderStatus::Rejected => "rejected",
        }
    }
}

impl TryFrom<&str> for OrderStatus {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "open" => Ok(OrderStatus::Open),
            "partially_filled" => Ok(OrderStatus::PartiallyFilled),
            "filled" => Ok(OrderStatus::Filled),
            "cancelled" => Ok(OrderStatus::Cancelled),
            "rejected" => Ok(OrderStatus::Rejected),
            other => Err(format!("invalid order status: {}", other)),
        }
    }
}

impl fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", <&str>::from(*self))
    }
}


// String conversions — STPMode


impl From<STPMode> for &str {
    fn from(mode: STPMode) -> &'static str {
        match mode {
            STPMode::CancelNewest => "cancel_newest",
            STPMode::CancelOldest => "cancel_oldest",
            STPMode::Decrement => "decrement",
            STPMode::None => "none",
        }
    }
}

impl TryFrom<&str> for STPMode {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "cancel_newest" => Ok(STPMode::CancelNewest),
            "cancel_oldest" => Ok(STPMode::CancelOldest),
            "decrement" => Ok(STPMode::Decrement),
            "none" => Ok(STPMode::None),
            other => Err(format!("invalid STP mode: {}", other)),
        }
    }
}

impl fmt::Display for STPMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", <&str>::from(*self))
    }
}


// FromStr impls (delegate to TryFrom<&str>)


impl FromStr for Side {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Side::try_from(s)
    }
}

impl FromStr for OrderType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        OrderType::try_from(s)
    }
}

impl FromStr for OrderStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        OrderStatus::try_from(s)
    }
}

impl FromStr for STPMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        STPMode::try_from(s)
    }
}
