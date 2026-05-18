//! Single-symbol limit order book with price-time priority.
//!
//! Two `BTreeMap<Price, PriceLevel>` instances hold resting orders — one
//! keyed by descending price for bids, one ascending for asks. Each price
//! level holds a `VecDeque<RestingOrder>` for FIFO time priority. An
//! `FxHashMap<OrderId, OrderLocation>` lets cancels find a resting order in
//! O(1) without scanning the book.
//!
//! `match_order` is the hot path. It pre-allocates nothing per call — the
//! caller passes in a `&mut Vec<Fill>` scratch buffer that is reused.

use std::collections::{BTreeMap, VecDeque};

use rustc_hash::FxHashMap;

use crate::protocol::{OrdType, Side, Tif};
use crate::{OrderId, Price, Qty, SymbolId};

#[derive(Debug, Clone, Copy)]
pub struct RestingOrder {
    pub order_id: OrderId,
    pub client_id: u64,
    pub side: Side,
    pub price: Price,
    pub qty_remaining: Qty,
}

#[derive(Debug, Default)]
pub struct PriceLevel {
    pub total_qty: Qty,
    pub queue: VecDeque<RestingOrder>,
}

#[derive(Debug, Clone, Copy)]
struct OrderLocation {
    side: Side,
    price: Price,
}

#[derive(Debug, Clone, Copy)]
pub struct Fill {
    pub maker_order_id: OrderId,
    pub taker_order_id: OrderId,
    pub maker_client_id: u64,
    pub taker_client_id: u64,
    pub price: Price,
    pub qty: Qty,
}

#[derive(Debug)]
pub struct OrderBook {
    pub symbol_id: SymbolId,
    /// Bids keyed by `-price` so that the lowest key is the best (highest) bid.
    bids: BTreeMap<Price, PriceLevel>,
    /// Asks keyed by `price` ascending — lowest key is the best ask.
    asks: BTreeMap<Price, PriceLevel>,
    orders: FxHashMap<OrderId, OrderLocation>,
}

impl OrderBook {
    pub fn new(symbol_id: SymbolId) -> Self {
        Self {
            symbol_id,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            orders: FxHashMap::default(),
        }
    }

    #[inline]
    pub fn best_bid(&self) -> Option<Price> {
        self.bids.keys().next().map(|k| -*k)
    }

    #[inline]
    pub fn best_ask(&self) -> Option<Price> {
        self.asks.keys().next().copied()
    }

    pub fn depth(&self, side: Side) -> usize {
        match side {
            Side::Buy => self.bids.len(),
            Side::Sell => self.asks.len(),
        }
    }

    pub fn order_count(&self) -> usize {
        self.orders.len()
    }

    /// Try to match an incoming order against resting orders on the opposite
    /// side, then rest the remainder (if any and TIF allows). Fills are
    /// appended to `fills`.
    ///
    /// Returns the quantity that ended up resting on the book (0 if the
    /// order fully filled, was cancelled by TIF, or was rejected).
    #[inline]
    pub fn submit(
        &mut self,
        order_id: OrderId,
        client_id: u64,
        side: Side,
        ord_type: OrdType,
        tif: Tif,
        limit_price: Price,
        qty: Qty,
        fills: &mut Vec<Fill>,
    ) -> SubmitOutcome {
        if qty == 0 {
            return SubmitOutcome::Rejected;
        }
        if self.orders.contains_key(&order_id) {
            return SubmitOutcome::Rejected;
        }

        // FOK pre-check: walk the opposite side without mutating and ensure
        // enough liquidity exists at acceptable prices.
        if tif == Tif::Fok && !self.has_liquidity(side, ord_type, limit_price, qty) {
            return SubmitOutcome::Rejected;
        }

        let mut remaining = qty;
        let crossed_filled = self.match_against(
            order_id,
            client_id,
            side,
            ord_type,
            limit_price,
            &mut remaining,
            fills,
        );

        // For market orders or IOC/FOK, never rest leftover.
        let rests = ord_type == OrdType::Limit && tif == Tif::Day && remaining > 0;
        if rests {
            self.rest(RestingOrder {
                order_id,
                client_id,
                side,
                price: limit_price,
                qty_remaining: remaining,
            });
            return SubmitOutcome::Resting {
                resting_qty: remaining,
                filled_qty: qty - remaining,
                crossed: crossed_filled,
            };
        }

        if remaining == qty {
            SubmitOutcome::NoFill
        } else if remaining == 0 {
            SubmitOutcome::FullyFilled
        } else {
            SubmitOutcome::PartialAndDone {
                filled_qty: qty - remaining,
            }
        }
    }

    fn has_liquidity(
        &self,
        taker_side: Side,
        ord_type: OrdType,
        limit_price: Price,
        qty: Qty,
    ) -> bool {
        let mut needed = qty;
        let opposite = match taker_side.opposite() {
            Side::Buy => &self.bids,
            Side::Sell => &self.asks,
        };
        for (key, level) in opposite.iter() {
            let level_price = if matches!(taker_side.opposite(), Side::Buy) {
                -*key
            } else {
                *key
            };
            if ord_type == OrdType::Limit {
                let acceptable = match taker_side {
                    Side::Buy => level_price <= limit_price,
                    Side::Sell => level_price >= limit_price,
                };
                if !acceptable {
                    break;
                }
            }
            if level.total_qty >= needed {
                return true;
            }
            needed -= level.total_qty;
        }
        false
    }

    #[inline]
    fn match_against(
        &mut self,
        taker_id: OrderId,
        taker_client: u64,
        taker_side: Side,
        ord_type: OrdType,
        limit_price: Price,
        remaining: &mut Qty,
        fills: &mut Vec<Fill>,
    ) -> bool {
        let mut any_filled = false;
        let opposite_is_bids = matches!(taker_side, Side::Sell);
        loop {
            if *remaining == 0 {
                break;
            }
            let book = if opposite_is_bids {
                &mut self.bids
            } else {
                &mut self.asks
            };
            let Some((&key, level)) = book.iter_mut().next() else {
                break;
            };
            let level_price = if opposite_is_bids { -key } else { key };
            if ord_type == OrdType::Limit {
                let acceptable = match taker_side {
                    Side::Buy => level_price <= limit_price,
                    Side::Sell => level_price >= limit_price,
                };
                if !acceptable {
                    break;
                }
            }

            while *remaining > 0 {
                let Some(maker) = level.queue.front_mut() else {
                    break;
                };
                let trade_qty = (*remaining).min(maker.qty_remaining);
                fills.push(Fill {
                    maker_order_id: maker.order_id,
                    taker_order_id: taker_id,
                    maker_client_id: maker.client_id,
                    taker_client_id: taker_client,
                    price: maker.price,
                    qty: trade_qty,
                });
                any_filled = true;
                *remaining -= trade_qty;
                maker.qty_remaining -= trade_qty;
                level.total_qty -= trade_qty;
                if maker.qty_remaining == 0 {
                    let removed = level.queue.pop_front().expect("front was Some");
                    self.orders.remove(&removed.order_id);
                }
            }

            if level.queue.is_empty() {
                book.remove(&key);
            }
        }
        any_filled
    }

    #[inline]
    fn rest(&mut self, order: RestingOrder) {
        let key = match order.side {
            Side::Buy => -order.price,
            Side::Sell => order.price,
        };
        let book = match order.side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };
        let level = book.entry(key).or_default();
        level.total_qty += order.qty_remaining;
        let order_id = order.order_id;
        let price = order.price;
        let side = order.side;
        level.queue.push_back(order);
        self.orders.insert(order_id, OrderLocation { side, price });
    }

    /// Remove a resting order. Returns the remaining quantity that was
    /// cancelled (0 if the order was not found).
    pub fn cancel(&mut self, order_id: OrderId) -> Qty {
        let Some(loc) = self.orders.remove(&order_id) else {
            return 0;
        };
        let key = match loc.side {
            Side::Buy => -loc.price,
            Side::Sell => loc.price,
        };
        let book = match loc.side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };
        let level = book.get_mut(&key).expect("index/level inconsistent");
        let mut removed = 0;
        if let Some(pos) = level
            .queue
            .iter()
            .position(|o| o.order_id == order_id)
        {
            let order = level.queue.remove(pos).expect("position was Some");
            removed = order.qty_remaining;
            level.total_qty -= removed;
        }
        if level.queue.is_empty() {
            book.remove(&key);
        }
        removed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmitOutcome {
    /// Order fully filled by matching.
    FullyFilled,
    /// Order partially filled, the remainder did not rest (IOC, FOK, market).
    PartialAndDone { filled_qty: Qty },
    /// No fills happened at all.
    NoFill,
    /// Order rests on the book (limit + day) — possibly after partial fill.
    Resting {
        resting_qty: Qty,
        filled_qty: Qty,
        crossed: bool,
    },
    /// Order rejected before any state change (zero qty, duplicate id, FOK
    /// without liquidity).
    Rejected,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PRICE_SCALE;

    fn px(p: i64) -> Price {
        p * PRICE_SCALE
    }

    fn buy_limit_day(book: &mut OrderBook, id: u64, price: i64, qty: u64) {
        let mut fills = Vec::new();
        book.submit(
            id,
            id,
            Side::Buy,
            OrdType::Limit,
            Tif::Day,
            px(price),
            qty,
            &mut fills,
        );
    }

    fn sell_limit_day(book: &mut OrderBook, id: u64, price: i64, qty: u64) {
        let mut fills = Vec::new();
        book.submit(
            id,
            id,
            Side::Sell,
            OrdType::Limit,
            Tif::Day,
            px(price),
            qty,
            &mut fills,
        );
    }

    #[test]
    fn resting_orders_set_best_prices() {
        let mut book = OrderBook::new(0);
        buy_limit_day(&mut book, 1, 99, 10);
        buy_limit_day(&mut book, 2, 100, 5);
        sell_limit_day(&mut book, 3, 101, 7);
        sell_limit_day(&mut book, 4, 102, 4);
        assert_eq!(book.best_bid(), Some(px(100)));
        assert_eq!(book.best_ask(), Some(px(101)));
    }

    #[test]
    fn marketable_buy_fills_against_best_ask_first() {
        let mut book = OrderBook::new(0);
        sell_limit_day(&mut book, 1, 102, 10);
        sell_limit_day(&mut book, 2, 101, 5);

        let mut fills = Vec::new();
        let outcome = book.submit(
            10,
            10,
            Side::Buy,
            OrdType::Limit,
            Tif::Day,
            px(102),
            7,
            &mut fills,
        );
        // 5 @ 101 + 2 @ 102 = 7
        assert_eq!(fills.len(), 2);
        assert_eq!(fills[0].price, px(101));
        assert_eq!(fills[0].qty, 5);
        assert_eq!(fills[1].price, px(102));
        assert_eq!(fills[1].qty, 2);
        assert_eq!(outcome, SubmitOutcome::FullyFilled);
        assert_eq!(book.best_ask(), Some(px(102)));
    }

    #[test]
    fn fifo_within_a_price_level() {
        let mut book = OrderBook::new(0);
        sell_limit_day(&mut book, 1, 100, 3);
        sell_limit_day(&mut book, 2, 100, 4);
        sell_limit_day(&mut book, 3, 100, 5);

        let mut fills = Vec::new();
        book.submit(
            10,
            10,
            Side::Buy,
            OrdType::Limit,
            Tif::Day,
            px(100),
            6,
            &mut fills,
        );
        assert_eq!(fills.len(), 2);
        assert_eq!(fills[0].maker_order_id, 1);
        assert_eq!(fills[0].qty, 3);
        assert_eq!(fills[1].maker_order_id, 2);
        assert_eq!(fills[1].qty, 3);
    }

    #[test]
    fn cancel_removes_order_and_empties_level() {
        let mut book = OrderBook::new(0);
        buy_limit_day(&mut book, 1, 99, 10);
        assert_eq!(book.depth(Side::Buy), 1);
        let removed = book.cancel(1);
        assert_eq!(removed, 10);
        assert_eq!(book.depth(Side::Buy), 0);
        assert_eq!(book.order_count(), 0);
        assert_eq!(book.best_bid(), None);
    }

    #[test]
    fn fok_rejects_when_insufficient_liquidity() {
        let mut book = OrderBook::new(0);
        sell_limit_day(&mut book, 1, 100, 3);
        let mut fills = Vec::new();
        let outcome = book.submit(
            10,
            10,
            Side::Buy,
            OrdType::Limit,
            Tif::Fok,
            px(100),
            5,
            &mut fills,
        );
        assert_eq!(outcome, SubmitOutcome::Rejected);
        assert!(fills.is_empty());
        // Book unchanged.
        assert_eq!(book.best_ask(), Some(px(100)));
    }

    #[test]
    fn ioc_partial_then_done() {
        let mut book = OrderBook::new(0);
        sell_limit_day(&mut book, 1, 100, 3);
        let mut fills = Vec::new();
        let outcome = book.submit(
            10,
            10,
            Side::Buy,
            OrdType::Limit,
            Tif::Ioc,
            px(100),
            5,
            &mut fills,
        );
        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0].qty, 3);
        assert_eq!(outcome, SubmitOutcome::PartialAndDone { filled_qty: 3 });
        // 2 remaining did NOT rest.
        assert_eq!(book.depth(Side::Buy), 0);
    }

    #[test]
    fn duplicate_id_rejected() {
        let mut book = OrderBook::new(0);
        buy_limit_day(&mut book, 1, 99, 5);
        let mut fills = Vec::new();
        let outcome = book.submit(
            1,
            1,
            Side::Buy,
            OrdType::Limit,
            Tif::Day,
            px(99),
            5,
            &mut fills,
        );
        assert_eq!(outcome, SubmitOutcome::Rejected);
    }
}
