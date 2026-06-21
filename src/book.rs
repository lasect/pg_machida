use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::types::{BookLevel, Order, Side};

pub fn price_to_tick(price: Decimal) -> usize {
    let scaled = price * Decimal::new(100, 0);
    scaled.to_u64().unwrap_or(0) as usize
}

pub fn tick_to_decimal(tick: usize) -> Decimal {
    Decimal::new(tick as i64, 2)
}

fn decimal_to_tick_qty(d: Decimal) -> u64 {
    d.to_u64().unwrap_or(0)
}

pub struct TickArray {
    pub qty: Vec<AtomicU64>,
    pub best_tick: AtomicU64,
}

impl TickArray {
    pub fn new(max_ticks: usize, initial_best_tick: u64) -> Self {
        TickArray {
            qty: (0..max_ticks).map(|_| AtomicU64::new(0)).collect(),
            best_tick: AtomicU64::new(initial_best_tick),
        }
    }

    pub fn len(&self) -> usize {
        self.qty.len()
    }

    pub fn add_qty(&self, tick: usize, amount: u64) {
        self.qty[tick].fetch_add(amount, Ordering::Release);
    }

    pub fn sub_qty(&self, tick: usize, amount: u64) {
        self.qty[tick].fetch_sub(amount, Ordering::Release);
    }

    pub fn get_qty(&self, tick: usize) -> u64 {
        self.qty[tick].load(Ordering::Acquire)
    }

    pub fn set_best_tick(&self, tick: usize) {
        self.best_tick.store(tick as u64, Ordering::Release);
    }

    pub fn get_best_tick(&self) -> usize {
        self.best_tick.load(Ordering::Acquire) as usize
    }

    pub fn update_best_bid(&self) {
        let mut tick = self.get_best_tick();
        loop {
            if self.get_qty(tick) > 0 {
                self.set_best_tick(tick);
                return;
            }
            if tick == 0 {
                break;
            }
            tick -= 1;
        }
        self.set_best_tick(0);
    }

    pub fn update_best_ask(&self) {
        let max = self.len();
        let mut tick = self.get_best_tick();
        loop {
            if self.get_qty(tick) > 0 {
                self.set_best_tick(tick);
                return;
            }
            tick += 1;
            if tick >= max {
                break;
            }
        }
        self.set_best_tick(max.saturating_sub(1));
    }
}

pub struct PriceLevel {
    pub orders: VecDeque<Order>,
    pub total_qty: Decimal,
}

impl PriceLevel {
    pub fn new() -> Self {
        PriceLevel {
            orders: VecDeque::new(),
            total_qty: Decimal::ZERO,
        }
    }

    pub fn remove_order(&mut self, order_id: Uuid) -> Option<Order> {
        let pos = self.orders.iter().position(|o| o.id == order_id)?;
        let order = self.orders.remove(pos)?;
        self.total_qty -= order.remaining;
        Some(order)
    }
}

pub struct OrderBook {
    pub max_ticks: usize,
    pub bids: TickArray,
    pub asks: TickArray,
    pub bid_levels: HashMap<usize, PriceLevel>,
    pub ask_levels: HashMap<usize, PriceLevel>,
    pub order_index: HashMap<Uuid, (Side, usize)>,
}

impl OrderBook {
    pub fn new(max_ticks: usize) -> Self {
        OrderBook {
            max_ticks,
            bids: TickArray::new(max_ticks, 0),
            asks: TickArray::new(max_ticks, max_ticks.saturating_sub(1) as u64),
            bid_levels: HashMap::new(),
            ask_levels: HashMap::new(),
            order_index: HashMap::new(),
        }
    }

    pub fn insert(&mut self, order: Order) {
        let tick = match order.price {
            Some(p) => price_to_tick(p),
            None => return,
        };
        let id = order.id;
        let side = order.side;
        let remaining = order.remaining;
        let tick_qty = decimal_to_tick_qty(remaining);

        match side {
            Side::Buy => {
                let level = self
                    .bid_levels
                    .entry(tick)
                    .or_insert_with(PriceLevel::new);
                level.total_qty += remaining;
                level.orders.push_back(order);
                self.bids.add_qty(tick, tick_qty);
                let best = self.bids.get_best_tick();
                if tick > best || self.bids.get_qty(best) == 0 {
                    self.bids.set_best_tick(tick);
                }
            }
            Side::Sell => {
                let level = self
                    .ask_levels
                    .entry(tick)
                    .or_insert_with(PriceLevel::new);
                level.total_qty += remaining;
                level.orders.push_back(order);
                self.asks.add_qty(tick, tick_qty);
                let best = self.asks.get_best_tick();
                if tick < best || self.asks.get_qty(best) == 0 {
                    self.asks.set_best_tick(tick);
                }
            }
        }

        self.order_index.insert(id, (side, tick));
    }

    pub fn cancel(&mut self, order_id: Uuid) -> Option<Order> {
        let (side, tick) = self.order_index.remove(&order_id)?;

        let order = match side {
            Side::Buy => self
                .bid_levels
                .get_mut(&tick)
                .and_then(|level| level.remove_order(order_id)),
            Side::Sell => self
                .ask_levels
                .get_mut(&tick)
                .and_then(|level| level.remove_order(order_id)),
        }?;

        let tick_qty = decimal_to_tick_qty(order.remaining);

        match side {
            Side::Buy => {
                self.bids.sub_qty(tick, tick_qty);
                let level_empty = self
                    .bid_levels
                    .get(&tick)
                    .map(|l| l.orders.is_empty())
                    .unwrap_or(true);
                if level_empty {
                    self.bid_levels.remove(&tick);
                    if self.bids.get_best_tick() == tick {
                        self.bids.update_best_bid();
                    }
                }
            }
            Side::Sell => {
                self.asks.sub_qty(tick, tick_qty);
                let level_empty = self
                    .ask_levels
                    .get(&tick)
                    .map(|l| l.orders.is_empty())
                    .unwrap_or(true);
                if level_empty {
                    self.ask_levels.remove(&tick);
                    if self.asks.get_best_tick() == tick {
                        self.asks.update_best_ask();
                    }
                }
            }
        }

        Some(order)
    }

    pub fn best_bid(&self) -> Option<Decimal> {
        let tick = self.bids.get_best_tick();
        if self.bids.get_qty(tick) > 0 {
            Some(tick_to_decimal(tick))
        } else {
            None
        }
    }

    pub fn best_ask(&self) -> Option<Decimal> {
        let tick = self.asks.get_best_tick();
        if self.asks.get_qty(tick) > 0 {
            Some(tick_to_decimal(tick))
        } else {
            None
        }
    }

    pub fn depth(&self, side: Side, levels: usize) -> Vec<BookLevel> {
        let mut result = Vec::new();

        match side {
            Side::Buy => {
                let mut tick = self.bids.get_best_tick();
                let mut collected = 0;
                loop {
                    let qty = self.bids.get_qty(tick);
                    if qty > 0 {
                        let order_count = self
                            .bid_levels
                            .get(&tick)
                            .map(|l| l.orders.len() as u32)
                            .unwrap_or(0);
                        result.push(BookLevel {
                            price: tick_to_decimal(tick),
                            qty: Decimal::new(qty as i64, 0),
                            order_count,
                        });
                        collected += 1;
                        if collected >= levels {
                            break;
                        }
                    }
                    if tick == 0 {
                        break;
                    }
                    tick -= 1;
                }
            }
            Side::Sell => {
                let mut tick = self.asks.get_best_tick();
                let mut collected = 0;
                let max = self.max_ticks;
                loop {
                    let qty = self.asks.get_qty(tick);
                    if qty > 0 {
                        let order_count = self
                            .ask_levels
                            .get(&tick)
                            .map(|l| l.orders.len() as u32)
                            .unwrap_or(0);
                        result.push(BookLevel {
                            price: tick_to_decimal(tick),
                            qty: Decimal::new(qty as i64, 0),
                            order_count,
                        });
                        collected += 1;
                        if collected >= levels {
                            break;
                        }
                    }
                    tick += 1;
                    if tick >= max {
                        break;
                    }
                }
            }
        }

        result
    }
}
