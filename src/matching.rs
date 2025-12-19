use crate::models::BalanceError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, VecDeque};
use uuid::Uuid;

// 订单状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OrderStatus {
    Pending,   // 等待撮合
    Partial,   // 部分成交
    Filled,    // 完全成交
    Cancelled, // 已取消
}

// 订单类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OrderType {
    Limit = 0,  // 限价单
    Market = 1, // 市价单
}

impl From<i32> for OrderType {
    fn from(value: i32) -> Self {
        match value {
            0 => OrderType::Limit,
            1 => OrderType::Market,
            _ => OrderType::Limit, // 默认限价单
        }
    }
}

// 订单方向
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OrderSide {
    Bid = 0, // 买入
    Ask = 1, // 卖出
}

impl From<i32> for OrderSide {
    fn from(value: i32) -> Self {
        match value {
            0 => OrderSide::Bid,
            1 => OrderSide::Ask,
            _ => OrderSide::Bid, // 默认买入
        }
    }
}

// 订单结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: u64,
    pub request_id: Uuid,
    pub symbol_id: i32,
    pub account_id: i32,
    pub order_type: OrderType,
    pub side: OrderSide,
    pub price: Decimal,
    pub quantity: Decimal,
    pub filled_quantity: Decimal,
    pub status: OrderStatus,
    pub created_at: u64, // 时间戳
}

impl Order {
    pub fn new(
        id: u64,
        request_id: Uuid,
        symbol_id: i32,
        account_id: i32,
        order_type: OrderType,
        side: OrderSide,
        price: Decimal,
        quantity: Decimal,
    ) -> Self {
        Self {
            id,
            request_id,
            symbol_id,
            account_id,
            order_type,
            side,
            price,
            quantity,
            filled_quantity: Decimal::ZERO,
            status: OrderStatus::Pending,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }
    }

    pub fn remaining_quantity(&self) -> Decimal {
        self.quantity - self.filled_quantity
    }

    pub fn is_filled(&self) -> bool {
        self.filled_quantity >= self.quantity
    }

    pub fn can_match(&self, other: &Order) -> bool {
        // 检查基本条件
        if self.symbol_id != other.symbol_id || self.side == other.side {
            return false;
        }

        // 检查价格匹配
        match (&self.side, &other.side) {
            (OrderSide::Bid, OrderSide::Ask) => {
                // 买单价格 >= 卖单价格
                self.price >= other.price
            }
            (OrderSide::Ask, OrderSide::Bid) => {
                // 卖单价格 <= 买单价格
                self.price <= other.price
            }
            _ => false,
        }
    }
}

// 成交记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub id: u64,
    pub symbol_id: i32,
    pub buy_order_id: u64,
    pub sell_order_id: u64,
    pub buy_account_id: i32,
    pub sell_account_id: i32,
    pub price: Decimal,
    pub quantity: Decimal,
    pub created_at: u64,
}

// 价格级别
#[derive(Debug, Clone)]
pub struct PriceLevel {
    pub price: Decimal,
    pub total_quantity: Decimal,
    pub orders: VecDeque<Order>,
}

impl PriceLevel {
    pub fn new(price: Decimal) -> Self {
        Self {
            price,
            total_quantity: Decimal::ZERO,
            orders: VecDeque::new(),
        }
    }

    pub fn add_order(&mut self, order: Order) {
        self.total_quantity += order.remaining_quantity();
        self.orders.push_back(order);
    }

    pub fn remove_order(&mut self, order_id: u64) -> Option<Order> {
        if let Some(pos) = self.orders.iter().position(|o| o.id == order_id) {
            let order = self.orders.remove(pos).unwrap();
            self.total_quantity -= order.remaining_quantity();
            Some(order)
        } else {
            None
        }
    }

    pub fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }

    pub fn update_quantity(&mut self) {
        self.total_quantity = self.orders.iter().map(|o| o.remaining_quantity()).sum();
    }
}

// 订单簿
#[derive(Debug, Clone)]
pub struct OrderBook {
    pub symbol_id: i32,
    pub bids: BTreeMap<Decimal, PriceLevel>, // 买单，按价格降序
    pub asks: BTreeMap<Decimal, PriceLevel>, // 卖单，按价格升序
    pub orders: HashMap<u64, Order>,         // 所有订单的索引
}

impl OrderBook {
    pub fn new(symbol_id: i32) -> Self {
        Self {
            symbol_id,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            orders: HashMap::new(),
        }
    }

    pub fn add_order(&mut self, mut order: Order) -> Vec<Trade> {
        let mut trades = Vec::new();

        // 尝试撮合
        if order.order_type == OrderType::Market {
            trades.extend(self.match_market_order(&mut order));
        } else {
            trades.extend(self.match_limit_order(&mut order));
        }

        // 如果订单还有剩余数量且不是市价单，添加到订单簿
        if order.remaining_quantity() > Decimal::ZERO && order.order_type == OrderType::Limit {
            self.add_order_to_book(order.clone());
        }

        // 更新订单状态
        if order.filled_quantity > Decimal::ZERO {
            if order.is_filled() {
                order.status = OrderStatus::Filled;
            } else {
                order.status = OrderStatus::Partial;
            }
        }

        self.orders.insert(order.id, order);
        trades
    }

    fn match_market_order(&mut self, order: &mut Order) -> Vec<Trade> {
        let mut trades = Vec::new();

        match order.side {
            OrderSide::Bid => {
                // 市价买单，从最优卖价开始撮合
                while order.remaining_quantity() > Decimal::ZERO && !self.asks.is_empty() {
                    let best_price = *self.asks.keys().next().unwrap();
                    if let Some(trade) = self.match_at_price(order, best_price) {
                        trades.push(trade);
                    } else {
                        break;
                    }
                }
            }
            OrderSide::Ask => {
                // 市价卖单，从最优买价开始撮合
                while order.remaining_quantity() > Decimal::ZERO && !self.bids.is_empty() {
                    let best_price = *self.bids.keys().next_back().unwrap();
                    if let Some(trade) = self.match_at_price(order, best_price) {
                        trades.push(trade);
                    } else {
                        break;
                    }
                }
            }
        }

        trades
    }

    fn match_limit_order(&mut self, order: &mut Order) -> Vec<Trade> {
        let mut trades = Vec::new();

        match order.side {
            OrderSide::Bid => {
                // 限价买单，撮合所有价格 <= 买单价格的卖单
                let mut prices_to_match: Vec<Decimal> = self
                    .asks
                    .keys()
                    .filter(|&&price| price <= order.price)
                    .cloned()
                    .collect();
                prices_to_match.sort();

                for price in prices_to_match {
                    if order.remaining_quantity() <= Decimal::ZERO {
                        break;
                    }
                    if let Some(trade) = self.match_at_price(order, price) {
                        trades.push(trade);
                    }
                }
            }
            OrderSide::Ask => {
                // 限价卖单，撮合所有价格 >= 卖单价格的买单
                let mut prices_to_match: Vec<Decimal> = self
                    .bids
                    .keys()
                    .filter(|&&price| price >= order.price)
                    .cloned()
                    .collect();
                prices_to_match.sort_by(|a, b| b.cmp(a)); // 降序

                for price in prices_to_match {
                    if order.remaining_quantity() <= Decimal::ZERO {
                        break;
                    }
                    if let Some(trade) = self.match_at_price(order, price) {
                        trades.push(trade);
                    }
                }
            }
        }

        trades
    }

    fn match_at_price(&mut self, taker_order: &mut Order, price: Decimal) -> Option<Trade> {
        // Generate trade ID first to avoid borrowing issues
        let trade_id = self.generate_trade_id();

        let book = match taker_order.side {
            OrderSide::Bid => &mut self.asks,
            OrderSide::Ask => &mut self.bids,
        };

        if let Some(price_level) = book.get_mut(&price) {
            if let Some(mut maker_order) = price_level.orders.pop_front() {
                let trade_quantity = taker_order
                    .remaining_quantity()
                    .min(maker_order.remaining_quantity());

                // 更新订单成交量
                taker_order.filled_quantity += trade_quantity;
                maker_order.filled_quantity += trade_quantity;

                // 创建成交记录
                let (buy_order_id, sell_order_id, buy_account_id, sell_account_id) =
                    match taker_order.side {
                        OrderSide::Bid => (
                            taker_order.id,
                            maker_order.id,
                            taker_order.account_id,
                            maker_order.account_id,
                        ),
                        OrderSide::Ask => (
                            maker_order.id,
                            taker_order.id,
                            maker_order.account_id,
                            taker_order.account_id,
                        ),
                    };

                let trade = Trade {
                    id: trade_id,
                    symbol_id: taker_order.symbol_id,
                    buy_order_id,
                    sell_order_id,
                    buy_account_id,
                    sell_account_id,
                    price,
                    quantity: trade_quantity,
                    created_at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64,
                };

                // 更新 maker 订单状态
                if maker_order.is_filled() {
                    maker_order.status = OrderStatus::Filled;
                } else {
                    maker_order.status = OrderStatus::Partial;
                    // 如果 maker 订单还有剩余，放回订单簿
                    price_level.orders.push_front(maker_order.clone());
                }

                // 更新订单索引
                self.orders.insert(maker_order.id, maker_order);

                // 更新价格级别
                price_level.update_quantity();

                // 如果价格级别为空，移除它
                if price_level.is_empty() {
                    book.remove(&price);
                }

                Some(trade)
            } else {
                None
            }
        } else {
            None
        }
    }

    fn add_order_to_book(&mut self, order: Order) {
        let book = match order.side {
            OrderSide::Bid => &mut self.bids,
            OrderSide::Ask => &mut self.asks,
        };

        book.entry(order.price)
            .or_insert_with(|| PriceLevel::new(order.price))
            .add_order(order);
    }

    pub fn cancel_order(&mut self, order_id: u64) -> Option<Order> {
        if let Some(order) = self.orders.get(&order_id).cloned() {
            let book = match order.side {
                OrderSide::Bid => &mut self.bids,
                OrderSide::Ask => &mut self.asks,
            };

            if let Some(price_level) = book.get_mut(&order.price) {
                if let Some(mut cancelled_order) = price_level.remove_order(order_id) {
                    cancelled_order.status = OrderStatus::Cancelled;
                    self.orders.insert(order_id, cancelled_order.clone());

                    // 如果价格级别为空，移除它
                    if price_level.is_empty() {
                        book.remove(&order.price);
                    }

                    return Some(cancelled_order);
                }
            }

            self.orders.remove(&order_id);
        }
        None
    }

    pub fn get_best_bid(&self) -> Option<Decimal> {
        self.bids.keys().next_back().cloned()
    }

    pub fn get_best_ask(&self) -> Option<Decimal> {
        self.asks.keys().next().cloned()
    }

    pub fn get_spread(&self) -> Option<Decimal> {
        if let (Some(best_bid), Some(best_ask)) = (self.get_best_bid(), self.get_best_ask()) {
            Some(best_ask - best_bid)
        } else {
            None
        }
    }

    fn generate_trade_id(&self) -> u64 {
        // 简单的 trade ID 生成，实际应用中可能需要更复杂的方案
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }

    pub fn get_market_depth(
        &self,
        levels: usize,
    ) -> (Vec<(Decimal, Decimal)>, Vec<(Decimal, Decimal)>) {
        let bids: Vec<(Decimal, Decimal)> = self
            .bids
            .iter()
            .rev()
            .take(levels)
            .map(|(price, level)| (*price, level.total_quantity))
            .collect();

        let asks: Vec<(Decimal, Decimal)> = self
            .asks
            .iter()
            .take(levels)
            .map(|(price, level)| (*price, level.total_quantity))
            .collect();

        (bids, asks)
    }
}

// 撮合引擎
#[derive(Debug)]
pub struct MatchingEngine {
    pub order_books: HashMap<i32, OrderBook>,
    pub next_order_id: u64,
    pub trades: Vec<Trade>,
}

impl MatchingEngine {
    pub fn new() -> Self {
        Self {
            order_books: HashMap::new(),
            next_order_id: 1,
            trades: Vec::new(),
        }
    }

    pub fn place_order(
        &mut self,
        request_id: Uuid,
        symbol_id: i32,
        account_id: i32,
        order_type: i32,
        side: i32,
        price_str: &str,
        quantity_str: &str,
    ) -> Result<(u64, Vec<Trade>), BalanceError> {
        // 解析价格和数量
        let quantity = Decimal::from_str_exact(quantity_str)
            .map_err(|_| BalanceError::InvalidAmount("Invalid quantity format".to_string()))?;

        let order_type = OrderType::from(order_type);
        let side = OrderSide::from(side);

        let price = if order_type == OrderType::Market {
            // 市价单使用特殊价格
            match side {
                OrderSide::Bid => Decimal::MAX,
                OrderSide::Ask => Decimal::ZERO,
            }
        } else {
            Decimal::from_str_exact(price_str)
                .map_err(|_| BalanceError::InvalidAmount("Invalid price format".to_string()))?
        };

        // 生成订单ID
        let order_id = self.next_order_id;
        self.next_order_id += 1;

        // 创建订单
        let order = Order::new(
            order_id, request_id, symbol_id, account_id, order_type, side, price, quantity,
        );

        // 获取或创建订单簿
        let order_book = self
            .order_books
            .entry(symbol_id)
            .or_insert_with(|| OrderBook::new(symbol_id));

        // 执行撮合
        let trades = order_book.add_order(order);

        // 保存成交记录
        for trade in &trades {
            self.trades.push(trade.clone());
        }

        Ok((order_id, trades))
    }

    pub fn cancel_order(&mut self, symbol_id: i32, order_id: u64) -> Option<Order> {
        self.order_books.get_mut(&symbol_id)?.cancel_order(order_id)
    }

    pub fn get_order_book(&self, symbol_id: i32) -> Option<&OrderBook> {
        self.order_books.get(&symbol_id)
    }

    pub fn get_recent_trades(&self, symbol_id: i32, limit: usize) -> Vec<&Trade> {
        self.trades
            .iter()
            .rev()
            .filter(|trade| trade.symbol_id == symbol_id)
            .take(limit)
            .collect()
    }
}