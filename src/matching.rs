use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::book::*;
use crate::types::*;

fn contra_front_remaining(
    book: &OrderBook,
    contra_side: Side,
    tick: usize,
) -> Option<Decimal> {
    let level = match contra_side {
        Side::Buy => book.bid_levels.get(&tick)?,
        Side::Sell => book.ask_levels.get(&tick)?,
    };
    level.front().map(|o| o.remaining)
}

fn contra_front_info(
    book: &OrderBook,
    contra_side: Side,
    tick: usize,
) -> Option<(Decimal, Uuid, String)> {
    let level = match contra_side {
        Side::Buy => book.bid_levels.get(&tick)?,
        Side::Sell => book.ask_levels.get(&tick)?,
    };
    level
        .front()
        .map(|o| (o.remaining, o.id, o.participant_id.clone()))
}

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
    let limit = order.price;

    match order.side {
        Side::Buy => {
            let mut ticks: Vec<usize> = book.ask_levels.keys().copied().collect();
            ticks.sort_unstable();
            for tick in ticks {
                let price = tick_to_decimal(tick);
                if let Some(limit_price) = limit {
                    if price > limit_price {
                        break;
                    }
                }
                if let Some(level) = book.ask_levels.get(&tick) {
                    needed -= level.total_qty;
                    if needed <= Decimal::ZERO {
                        return true;
                    }
                }
            }
        }
        Side::Sell => {
            let mut ticks: Vec<usize> = book.bid_levels.keys().copied().collect();
            ticks.sort_unstable_by(|a, b| b.cmp(a));
            for tick in ticks {
                let price = tick_to_decimal(tick);
                if let Some(limit_price) = limit {
                    if price < limit_price {
                        break;
                    }
                }
                if let Some(level) = book.bid_levels.get(&tick) {
                    needed -= level.total_qty;
                    if needed <= Decimal::ZERO {
                        return true;
                    }
                }
            }
        }
    }

    needed <= Decimal::ZERO
}

pub fn match_order(book: &mut OrderBook, mut order: Order) -> PlaceOrderResult {
    if order.qty <= Decimal::ZERO {
        return PlaceOrderResult::rejected(order.id);
    }
    if let Some(price) = order.price {
        if price <= Decimal::ZERO {
            return PlaceOrderResult::rejected(order.id);
        }
        let scaled = price * Decimal::new(100, 0);
        if scaled.fract() != Decimal::ZERO
            || scaled
                .to_u64()
                .map_or(true, |t| t as usize >= book.max_ticks)
        {
            return PlaceOrderResult::rejected(order.id);
        }
    }

    let mut trades = Vec::new();
    let mut filled_qty = Decimal::ZERO;
    let mut weighted_price_sum = Decimal::ZERO;
    let mut fill_seq: u64 = 0;

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
                    let Some(resting_qty) =
                        contra_front_remaining(book, contra_side, best_tick)
                    else {
                        break;
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
        let Some((resting_qty, resting_id, resting_participant)) =
            contra_front_info(book, contra_side, best_tick)
        else {
            break;
        };

        let fill_qty = order.remaining.min(resting_qty);

        let buy_id = if matches!(order.side, Side::Buy) {
            order.id
        } else {
            resting_id
        };
        let sell_id = if matches!(order.side, Side::Sell) {
            order.id
        } else {
            resting_id
        };
        let buy_participant = if matches!(order.side, Side::Buy) {
            order.participant_id.clone()
        } else {
            resting_participant.clone()
        };
        let sell_participant = if matches!(order.side, Side::Sell) {
            order.participant_id.clone()
        } else {
            resting_participant
        };

        let trade_id =
            Trade::compute_id(order.instrument_id, buy_id, sell_id, best_price, fill_qty, fill_seq);
        fill_seq += 1;

        let trade = Trade {
            id: trade_id,
            instrument_id: order.instrument_id,
            buy_order_id: buy_id,
            sell_order_id: sell_id,
            buy_participant_id: buy_participant,
            sell_participant_id: sell_participant,
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
