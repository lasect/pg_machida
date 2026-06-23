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
    pub fn instrument_id(&self, symbol: &str) -> Option<u64> {
        self.symbol_map.get(symbol).copied()
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
