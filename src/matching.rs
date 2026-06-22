use rust_decimal::Decimal;
use uuid::Uuid;

use crate::book::*;
use crate::types::*;

fn remove_contra_front(book: &mut OrderBook, contra_side: Side, tick: usize) -> Option<Order> {
    let mut level = match contra_side {
        Side::Buy => book.bid_levels.remove(&tick)?,
        Side::Sell => book.ask_levels.remove(&tick)?,
    };

    if level.is_empty() {
        match contra_side {
            Side::Buy => {
                book.bid_levels.insert(tick, level);
            }
            Side::Sell => {
                book.ask_levels.insert(tick, level);
            }
        }
        return None;
    }

    let order = level.pop_front()?;

    book.order_index.remove(&order.id);
    let tick_qty = decimal_to_tick_qty(order.remaining);

    if !level.is_empty() {
        match contra_side {
            Side::Buy => {
                book.bids.sub_qty(tick, tick_qty);
                book.bid_levels.insert(tick, level);
            }
            Side::Sell => {
                book.asks.sub_qty(tick, tick_qty);
                book.ask_levels.insert(tick, level);
            }
        }
    } else {
        match contra_side {
            Side::Buy => {
                book.bids.sub_qty(tick, tick_qty);
                if book.bids.get_best_tick() == tick {
                    book.bids.update_best_bid();
                }
            }
            Side::Sell => {
                book.asks.sub_qty(tick, tick_qty);
                if book.asks.get_best_tick() == tick {
                    book.asks.update_best_ask();
                }
            }
        }
    }

    Some(order)
}

fn fill_contra_front(
    book: &mut OrderBook,
    contra_side: Side,
    tick: usize,
    fill_qty: Decimal,
) -> Option<Uuid> {
    let mut level = match contra_side {
        Side::Buy => book.bid_levels.remove(&tick)?,
        Side::Sell => book.ask_levels.remove(&tick)?,
    };

    if level.is_empty() {
        match contra_side {
            Side::Buy => {
                book.bid_levels.insert(tick, level);
            }
            Side::Sell => {
                book.ask_levels.insert(tick, level);
            }
        }
        return None;
    }

    level.reduce_front_remaining(fill_qty);

    let tick_qty = decimal_to_tick_qty(fill_qty);
    match contra_side {
        Side::Buy => book.bids.sub_qty(tick, tick_qty),
        Side::Sell => book.asks.sub_qty(tick, tick_qty),
    };

    let mut popped_id = None;

    if level.front().map_or(false, |o| o.remaining.is_zero()) {
        if let Some(order) = level.pop_front() {
            book.order_index.remove(&order.id);
            popped_id = Some(order.id);
        }
    }

    if !level.is_empty() {
        match contra_side {
            Side::Buy => {
                book.bid_levels.insert(tick, level);
            }
            Side::Sell => {
                book.ask_levels.insert(tick, level);
            }
        }
    } else {
        match contra_side {
            Side::Buy => {
                if book.bids.get_best_tick() == tick {
                    book.bids.update_best_bid();
                }
            }
            Side::Sell => {
                if book.asks.get_best_tick() == tick {
                    book.asks.update_best_ask();
                }
            }
        }
    }

    popped_id
}

fn can_fill_fully(book: &OrderBook, order: &Order) -> bool {
    let mut needed = order.qty;

    match order.side {
        Side::Buy => {
            let mut tick = book.asks.get_best_tick();
            let max = book.max_ticks;
            while needed > Decimal::ZERO && tick < max {
                if let Some(level) = book.ask_levels.get(&tick) {
                    needed -= level.total_qty;
                }
                tick += 1;
            }
        }
        Side::Sell => {
            let mut tick = book.bids.get_best_tick();
            loop {
                if let Some(level) = book.bid_levels.get(&tick) {
                    needed -= level.total_qty;
                }
                if tick == 0 || needed <= Decimal::ZERO {
                    break;
                }
                tick -= 1;
            }
        }
    }

    needed <= Decimal::ZERO
}

pub fn match_order(book: &mut OrderBook, mut order: Order) -> PlaceOrderResult {
    let mut trades = Vec::new();
    let mut filled_qty = Decimal::ZERO;
    let mut weighted_price_sum = Decimal::ZERO;

    // FOK pre-check: verify full qty available before starting
    if order.order_type == OrderType::FOK {
        if !can_fill_fully(book, &order) {
            return PlaceOrderResult::cancelled(order.id);
        }
    }

    loop {
        if order.remaining.is_zero() {
            break;
        }

        let contra_side = match order.side {
            Side::Buy => Side::Sell,
            Side::Sell => Side::Buy,
        };

        let best_tick = match order.side {
            Side::Buy => book.asks.get_best_tick(),
            Side::Sell => book.bids.get_best_tick(),
        };

        let best_qty = match order.side {
            Side::Buy => book.asks.get_qty(best_tick),
            Side::Sell => book.bids.get_qty(best_tick),
        };

        let has_orders = match order.side {
            Side::Buy => book
                .ask_levels
                .get(&best_tick)
                .map_or(false, |l| !l.is_empty()),
            Side::Sell => book
                .bid_levels
                .get(&best_tick)
                .map_or(false, |l| !l.is_empty()),
        };

        if best_qty == 0 && !has_orders {
            break;
        }

        let best_price = tick_to_decimal(best_tick);

        // Price check for limit orders
        if let Some(limit_price) = order.price {
            let crosses = match order.side {
                Side::Buy => limit_price >= best_price,
                Side::Sell => limit_price <= best_price,
            };
            if !crosses {
                break;
            }
        }

        // STP check — peek at front order participant
        let stp_triggered = {
            let level = match order.side {
                Side::Buy => book.ask_levels.get(&best_tick),
                Side::Sell => book.bid_levels.get(&best_tick),
            };
            level
                .and_then(|l| l.front())
                .map_or(false, |resting| resting.participant_id == order.participant_id)
        };

        if stp_triggered {
            match order.stp_mode {
                STPMode::CancelNewest => {
                    return PlaceOrderResult {
                        order_id: order.id,
                        status: if filled_qty > Decimal::ZERO {
                            OrderStatus::PartiallyFilled
                        } else {
                            OrderStatus::Cancelled
                        },
                        filled_qty,
                        avg_fill_price: if filled_qty > Decimal::ZERO {
                            Some(weighted_price_sum / filled_qty)
                        } else {
                            None
                        },
                        trades,
                    };
                }
                STPMode::CancelOldest => {
                    remove_contra_front(book, contra_side, best_tick);
                    continue;
                }
                STPMode::Decrement => {
                    let resting_qty = {
                        let level = match order.side {
                            Side::Buy => book.ask_levels.get(&best_tick).unwrap(),
                            Side::Sell => book.bid_levels.get(&best_tick).unwrap(),
                        };
                        level.front().unwrap().remaining
                    };
                    if order.remaining <= resting_qty {
                        // Incoming is fully consumed by STP
                        remove_contra_front(book, contra_side, best_tick);
                        return PlaceOrderResult {
                            order_id: order.id,
                            status: if filled_qty > Decimal::ZERO {
                                OrderStatus::PartiallyFilled
                            } else {
                                OrderStatus::Cancelled
                            },
                            filled_qty,
                            avg_fill_price: if filled_qty > Decimal::ZERO {
                                Some(weighted_price_sum / filled_qty)
                            } else {
                                None
                            },
                            trades,
                        };
                    }
                    order.remaining -= resting_qty;
                    remove_contra_front(book, contra_side, best_tick);
                    continue;
                }
                STPMode::None => {
                    // Allow self-trade, fall through to fill
                }
            }
        }

        // Read resting order data before the mutable fill operation
        let (resting_qty, resting_id, resting_participant) = {
            let level = match order.side {
                Side::Buy => book.ask_levels.get(&best_tick).unwrap(),
                Side::Sell => book.bid_levels.get(&best_tick).unwrap(),
            };
            let resting = level.front().unwrap();
            (resting.remaining, resting.id, resting.participant_id.clone())
        };

        let fill_qty = order.remaining.min(resting_qty);

        let trade = Trade {
            id: Uuid::new_v4(),
            instrument_id: order.instrument_id,
            buy_order_id: if matches!(order.side, Side::Buy) {
                order.id
            } else {
                resting_id
            },
            sell_order_id: if matches!(order.side, Side::Sell) {
                order.id
            } else {
                resting_id
            },
            buy_participant_id: if matches!(order.side, Side::Buy) {
                order.participant_id.clone()
            } else {
                resting_participant.clone()
            },
            sell_participant_id: if matches!(order.side, Side::Sell) {
                order.participant_id.clone()
            } else {
                resting_participant
            },
            price: best_price,
            qty: fill_qty,
            ts: order.ts,
        };
        trades.push(trade);

        filled_qty += fill_qty;
        weighted_price_sum += best_price * fill_qty;
        order.remaining -= fill_qty;

        fill_contra_front(book, contra_side, best_tick, fill_qty);
    }

    // Capture fields before potential move into book.insert()
    let order_id = order.id;
    let remaining = order.remaining;
    let order_type = order.order_type;

    // Post-match: rest limit orders on book, cancel remainder for others
    if remaining > Decimal::ZERO {
        match order_type {
            OrderType::Limit => {
                order.status = if filled_qty > Decimal::ZERO {
                    OrderStatus::PartiallyFilled
                } else {
                    OrderStatus::Open
                };
                book.insert(order);
            }
            _ => {
                // Market, IOC, FOK — remainder is cancelled
            }
        }
    }

    let status = if filled_qty.is_zero() {
        if remaining.is_zero() {
            OrderStatus::Cancelled
        } else if order_type == OrderType::Limit {
            OrderStatus::Open
        } else {
            OrderStatus::Cancelled
        }
    } else if remaining.is_zero() {
        OrderStatus::Filled
    } else {
        OrderStatus::PartiallyFilled
    };

    PlaceOrderResult {
        order_id,
        status,
        filled_qty,
        avg_fill_price: if filled_qty > Decimal::ZERO {
            Some(weighted_price_sum / filled_qty)
        } else {
            None
        },
        trades,
    }
}
