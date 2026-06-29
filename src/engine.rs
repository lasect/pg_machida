use std::collections::HashMap;

use rust_decimal::Decimal;
use uuid::Uuid;

use crate::book::{price_to_tick, OrderBook};
use crate::error::ClobError;
use crate::matching::match_order;
use crate::types::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstrumentStatus {
    Active,
    Halted,
}

pub struct Instrument {
    pub id: u64,
    pub symbol: String,
    pub tick_size: Decimal,
    pub lot_size: Decimal,
    pub status: InstrumentStatus,
    pub circuit_breaker: Option<CircuitBreaker>,
    pub book: OrderBook,
}

#[derive(Clone, Debug)]
pub struct BookDepth {
    pub bids: Vec<BookLevel>,
    pub asks: Vec<BookLevel>,
}

pub struct ClobEngine {
    next_id: u64,
    instruments: HashMap<u64, Instrument>,
    symbol_map: HashMap<String, u64>,
    order_instrument: HashMap<Uuid, u64>,
}

impl ClobEngine {
    pub fn new() -> Self {
        ClobEngine {
            next_id: 1,
            instruments: HashMap::new(),
            symbol_map: HashMap::new(),
            order_instrument: HashMap::new(),
        }
    }

    pub fn create_instrument(
        &mut self,
        symbol: &str,
        tick_size: Decimal,
        lot_size: Decimal,
        max_ticks: usize,
    ) -> Result<u64, ClobError> {
        if symbol.is_empty() {
            return Err(ClobError::InstrumentNotFound(
                "symbol must not be empty".into(),
            ));
        }
        if self.symbol_map.contains_key(symbol) {
            return Err(ClobError::InvalidPrice(format!(
                "symbol '{}' already exists",
                symbol
            )));
        }
        if tick_size <= Decimal::ZERO {
            return Err(ClobError::InvalidPrice("tick_size must be > 0".into()));
        }
        if lot_size <= Decimal::ZERO {
            return Err(ClobError::InvalidQty("lot_size must be > 0".into()));
        }
        if max_ticks == 0 {
            return Err(ClobError::InvalidPrice("max_ticks must be > 0".into()));
        }

        let id = self.next_id;
        self.next_id += 1;

        let instrument = Instrument {
            id,
            symbol: symbol.to_string(),
            tick_size,
            lot_size,
            status: InstrumentStatus::Active,
            circuit_breaker: None,
            book: OrderBook::new(max_ticks),
        };

        self.instruments.insert(id, instrument);
        self.symbol_map.insert(symbol.to_string(), id);

        Ok(id)
    }
    pub fn create_instrument_with_id(
        &mut self,
        id: u64,
        symbol: &str,
        tick_size: Decimal,
        lot_size: Decimal,
        max_ticks: usize,
    ) -> Result<u64, ClobError> {
        if symbol.is_empty() {
            return Err(ClobError::InstrumentNotFound(
                "symbol must not be empty".into(),
            ));
        }
        if self.symbol_map.contains_key(symbol) {
            return Err(ClobError::InvalidPrice(format!(
                "symbol '{}' already exists",
                symbol
            )));
        }
        if self.instruments.contains_key(&id) {
            return Err(ClobError::InvalidPrice(format!(
                "instrument id {} already exists",
                id
            )));
        }
        if tick_size <= Decimal::ZERO {
            return Err(ClobError::InvalidPrice("tick_size must be > 0".into()));
        }
        if lot_size <= Decimal::ZERO {
            return Err(ClobError::InvalidQty("lot_size must be > 0".into()));
        }
        if max_ticks == 0 {
            return Err(ClobError::InvalidPrice("max_ticks must be > 0".into()));
        }

        self.next_id = self.next_id.max(id + 1);

        let instrument = Instrument {
            id,
            symbol: symbol.to_string(),
            tick_size,
            lot_size,
            status: InstrumentStatus::Active,
            circuit_breaker: None,
            book: OrderBook::new(max_ticks),
        };

        self.instruments.insert(id, instrument);
        self.symbol_map.insert(symbol.to_string(), id);

        Ok(id)
    }

    pub fn instrument_id(&self, symbol: &str) -> Option<u64> {
        self.symbol_map.get(symbol).copied()
    }

    /// Load an order directly into the book without running matching.
    /// Used during crash recovery to rebuild in-memory state from Postgres.
    pub fn load_order(&mut self, order: Order) -> Result<(), ClobError> {
        let instr = self
            .instruments
            .get_mut(&order.instrument_id)
            .ok_or_else(|| ClobError::InstrumentNotFound(order.instrument_id.to_string()))?;

        let order_id = order.id;
        instr.book.insert(order);
        self.order_instrument.insert(order_id, instr.id);
        Ok(())
    }

    pub fn place_order(&mut self, order: Order) -> Result<PlaceOrderResult, ClobError> {
        let instr = self
            .instruments
            .get_mut(&order.instrument_id)
            .ok_or_else(|| ClobError::InstrumentNotFound(order.instrument_id.to_string()))?;

        if instr.status == InstrumentStatus::Halted {
            return Err(ClobError::InstrumentHalted(instr.symbol.clone()));
        }

        // Tick-size validation (only for limit orders with a price)
        if let Some(price) = order.price {
            let divided = price / instr.tick_size;
            if divided.fract() != Decimal::ZERO {
                return Err(ClobError::InvalidPrice(format!(
                    "price {} is not a multiple of tick_size {}",
                    price, instr.tick_size
                )));
            }
            // Also validate the price maps to a tick within bounds
            let tick = price_to_tick(price);
            if tick >= instr.book.max_ticks {
                return Err(ClobError::InvalidPrice(format!(
                    "price {} exceeds max tick range",
                    price
                )));
            }
        }

        // Lot-size validation
        let divided = order.qty / instr.lot_size;
        if divided.fract() != Decimal::ZERO {
            return Err(ClobError::InvalidQty(format!(
                "quantity {} is not a multiple of lot_size {}",
                order.qty, instr.lot_size
            )));
        }

        let order_id = order.id;
        let result = match_order(&mut instr.book, order);

        // Clean up resting orders that were fully filled by this match
        for trade in &result.trades {
            if !instr.book.order_index.contains_key(&trade.buy_order_id) {
                self.order_instrument.remove(&trade.buy_order_id);
            }
            if !instr.book.order_index.contains_key(&trade.sell_order_id) {
                self.order_instrument.remove(&trade.sell_order_id);
            }
        }

        // Track resting orders in the engine-level index
        match result.status {
            OrderStatus::Open | OrderStatus::PartiallyFilled => {
                if instr.book.order_index.contains_key(&order_id) {
                    self.order_instrument.insert(order_id, instr.id);
                }
            }
            _ => {}
        }
        // Circuit-breaker check — halt instrument if any trade breaches threshold
        if let Some(ref cb) = instr.circuit_breaker {
            if result.trades.iter().any(|t| circuit_breached(cb, t.price)) {
                instr.status = InstrumentStatus::Halted;
            }
        }

        Ok(result)
    }

    pub fn cancel_order(&mut self, order_id: Uuid) -> Result<Order, ClobError> {
        let instr_id = *self
            .order_instrument
            .get(&order_id)
            .ok_or_else(|| ClobError::OrderNotFound(order_id.to_string()))?;

        let instr = self
            .instruments
            .get_mut(&instr_id)
            .ok_or_else(|| ClobError::InstrumentNotFound(instr_id.to_string()))?;

        let cancelled_order = instr
            .book
            .cancel(order_id)
            .ok_or_else(|| ClobError::OrderNotFound(order_id.to_string()))?;

        // Only remove the mapping if the cancellation was successful
        self.order_instrument.remove(&order_id);

        Ok(cancelled_order)
    }

    pub fn mass_cancel(
        &mut self,
        instrument_id: u64,
        participant_id: &str,
    ) -> Result<u32, ClobError> {
        let instr = self
            .instruments
            .get_mut(&instrument_id)
            .ok_or_else(|| ClobError::InstrumentNotFound(instrument_id.to_string()))?;

        let mut to_cancel = Vec::new();

        for (order_id, (_side, _tick)) in &instr.book.order_index {
            let is_match = instr
                .book
                .bid_levels
                .values()
                .flat_map(|l| l.orders.iter())
                .chain(instr.book.ask_levels.values().flat_map(|l| l.orders.iter()))
                .any(|o| o.id == *order_id && o.participant_id == participant_id);
            if is_match {
                to_cancel.push(*order_id);
            }
        }

        let count = to_cancel.len() as u32;
        for id in to_cancel {
            instr.book.cancel(id);
            self.order_instrument.remove(&id);
        }

        Ok(count)
    }

    pub fn get_open_orders(
        &self,
        instrument_id: u64,
        participant_id: &str,
    ) -> Result<Vec<Order>, ClobError> {
        let instr = self
            .instruments
            .get(&instrument_id)
            .ok_or_else(|| ClobError::InstrumentNotFound(instrument_id.to_string()))?;

        let orders: Vec<Order> = instr
            .book
            .bid_levels
            .values()
            .flat_map(|l| l.orders.iter())
            .chain(instr.book.ask_levels.values().flat_map(|l| l.orders.iter()))
            .filter(|o| o.participant_id == participant_id)
            .cloned()
            .collect();

        Ok(orders)
    }

    pub fn get_instrument_id(&self, symbol: &str) -> Option<u64> {
        self.instrument_id(symbol)
    }

    pub fn iter_instruments(&self) -> impl Iterator<Item = (&u64, &Instrument)> {
        self.instruments.iter()
    }

    pub fn get_book_depth(
        &self,
        instrument_id: u64,
        levels: usize,
    ) -> Result<BookDepth, ClobError> {
        let instr = self
            .instruments
            .get(&instrument_id)
            .ok_or_else(|| ClobError::InstrumentNotFound(instrument_id.to_string()))?;

        Ok(BookDepth {
            bids: instr.book.depth(Side::Buy, levels),
            asks: instr.book.depth(Side::Sell, levels),
        })
    }

    pub fn halt_instrument(&mut self, instrument_id: u64) -> Result<(), ClobError> {
        let instr = self
            .instruments
            .get_mut(&instrument_id)
            .ok_or_else(|| ClobError::InstrumentNotFound(instrument_id.to_string()))?;
        instr.status = InstrumentStatus::Halted;
        Ok(())
    }

    pub fn resume_instrument(&mut self, instrument_id: u64) -> Result<(), ClobError> {
        let instr = self
            .instruments
            .get_mut(&instrument_id)
            .ok_or_else(|| ClobError::InstrumentNotFound(instrument_id.to_string()))?;
        instr.status = InstrumentStatus::Active;
        Ok(())
    }

    pub fn set_circuit_breaker(
        &mut self,
        instrument_id: u64,
        cb: CircuitBreaker,
    ) -> Result<(), ClobError> {
        let instr = self
            .instruments
            .get_mut(&instrument_id)
            .ok_or_else(|| ClobError::InstrumentNotFound(instrument_id.to_string()))?;
        instr.circuit_breaker = Some(cb);
        Ok(())
    }

    pub fn instrument_status(&self, instrument_id: u64) -> Option<InstrumentStatus> {
        self.instruments.get(&instrument_id).map(|i| i.status)
    }

    pub fn best_bid(&self, instrument_id: u64) -> Option<Decimal> {
        self.instruments.get(&instrument_id)?.book.best_bid()
    }

    pub fn best_ask(&self, instrument_id: u64) -> Option<Decimal> {
        self.instruments.get(&instrument_id)?.book.best_ask()
    }

    pub fn order_count(&self) -> usize {
        self.order_instrument.len()
    }

    pub fn instrument_count(&self) -> usize {
        self.instruments.len()
    }

    pub fn all_instrument_ids(&self) -> Vec<u64> {
        self.instruments.keys().copied().collect()
    }

    pub fn instrument_symbol(&self, instrument_id: u64) -> Option<&str> {
        self.instruments.get(&instrument_id).map(|i| i.symbol.as_str())
    }

    /// Collect all resting (open / partially filled) orders from every
    /// instrument, sorted by `ts` ascending — the order they should be
    /// replayed on rebuild to preserve time priority.
    pub fn resting_orders(&self) -> Vec<Order> {
        let mut orders: Vec<Order> = Vec::new();

        for instr in self.instruments.values() {
            for level in instr.book.bid_levels.values() {
                for order in &level.orders {
                    orders.push(order.clone());
                }
            }
            for level in instr.book.ask_levels.values() {
                for order in &level.orders {
                    orders.push(order.clone());
                }
            }
        }

        orders.sort_by_key(|o| o.ts);
        orders
    }
}

fn circuit_breached(cb: &CircuitBreaker, trade_price: Decimal) -> bool {
    let diff = if trade_price >= cb.reference_price {
        trade_price - cb.reference_price
    } else {
        cb.reference_price - trade_price
    };
    let threshold = cb.reference_price * cb.halt_pct;
    diff >= threshold
}
