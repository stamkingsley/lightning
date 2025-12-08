use crate::matching::{MatchingEngine, Trade};
use crate::messages::{MatchMessage, SequencerMessage, TradeExecutionMessage};
use crate::models::{get_symbol, BalanceError};

pub struct SequencerProcessor {
    id: usize,
    receiver: crossbeam_channel::Receiver<SequencerMessage>,
    balance_manager: crate::models::BalanceManager,
    match_senders: Vec<crossbeam_channel::Sender<MatchMessage>>,
    trade_execution_receiver: crossbeam_channel::Receiver<TradeExecutionMessage>,
}

pub struct MatchProcessor {
    id: usize,
    receiver: crossbeam_channel::Receiver<MatchMessage>,
    matching_engine: MatchingEngine,
    sequencer_senders: Vec<crossbeam_channel::Sender<TradeExecutionMessage>>,
}

impl MatchProcessor {
    pub fn new(
        id: usize,
        receiver: crossbeam_channel::Receiver<MatchMessage>,
        sequencer_senders: Vec<crossbeam_channel::Sender<TradeExecutionMessage>>,
    ) -> Self {
        Self {
            id,
            receiver,
            matching_engine: MatchingEngine::new(),
            sequencer_senders,
        }
    }

    pub fn run(mut self) {
        println!("Match processor {} started", self.id);
        loop {
            match self.receiver.recv() {
                Ok(message) => match message {
                    MatchMessage::PlaceOrder {
                        request_id,
                        symbol_id,
                        account_id,
                        order_type,
                        side,
                        price,
                        quantity,
                        response_sender,
                    } => {
                        self.handle_place_order(
                            request_id,
                            symbol_id,
                            account_id,
                            order_type,
                            side,
                            price,
                            quantity,
                            response_sender,
                        );
                    }
                    MatchMessage::GetOrderBook {
                        request_id,
                        symbol_id,
                        levels,
                        response_sender,
                    } => {
                        self.handle_get_order_book(request_id, symbol_id, levels, response_sender);
                    }
                    MatchMessage::CancelOrder {
                        request_id,
                        symbol_id,
                        account_id,
                        order_id,
                        response_sender,
                    } => {
                        self.handle_cancel_order(
                            request_id,
                            symbol_id,
                            account_id,
                            order_id,
                            response_sender,
                        );
                    }
                },
                Err(_) => {
                    println!("Match processor {} stopped - channel closed", self.id);
                    break;
                }
            }
        }
    }

    fn handle_place_order(
        &mut self,
        request_id: uuid::Uuid,
        symbol_id: i32,
        account_id: i32,
        order_type: i32,
        side: i32,
        price: String,
        quantity: String,
        response_sender: tokio::sync::oneshot::Sender<crate::models::schema::PlaceOrderResponse>,
    ) {
        println!(
            "MatchProcessor {}: Processing order - symbol={}, account={}, type={}, side={}, price={}, quantity={}",
            self.id, symbol_id, account_id, order_type, side, price, quantity
        );

        // 执行撮合
        match self.matching_engine.place_order(
            request_id, symbol_id, account_id, order_type, side, &price, &quantity,
        ) {
            Ok((order_id, trades)) => {
                println!(
                    "MatchProcessor {}: Order {} placed successfully, {} trades generated",
                    self.id,
                    order_id,
                    trades.len()
                );

                // 如果有成交，发送成交记录到余额管理器执行
                if !trades.is_empty() {
                    self.execute_trades(trades, order_id, response_sender);
                } else {
                    // 没有成交，直接返回成功响应
                    let response = crate::models::schema::PlaceOrderResponse {
                        code: 0,
                        message: Some("Order placed successfully".to_string()),
                        id: order_id as i64,
                    };
                    let _ = response_sender.send(response);
                }

                // 显示当前市场深度
                if let Some(order_book) = self.matching_engine.get_order_book(symbol_id) {
                    let (bids, asks) = order_book.get_market_depth(5);
                    println!("Market depth for symbol {}:", symbol_id);
                    println!("  Bids: {:?}", bids);
                    println!("  Asks: {:?}", asks);
                    if let Some(spread) = order_book.get_spread() {
                        println!("  Spread: {}", spread);
                    }
                }
            }
            Err(e) => {
                println!("MatchProcessor {}: Order failed - {}", self.id, e);
                let response = crate::models::schema::PlaceOrderResponse {
                    code: 400,
                    message: Some(format!("Order failed: {}", e)),
                    id: 0,
                };
                let _ = response_sender.send(response);
            }
        }
    }

    fn execute_trades(
        &self,
        trades: Vec<Trade>,
        order_id: u64,
        response_sender: tokio::sync::oneshot::Sender<crate::models::schema::PlaceOrderResponse>,
    ) {
        println!(
            "MatchProcessor {}: Executing {} trades for order {}",
            self.id,
            trades.len(),
            order_id
        );

        // 发送每笔成交到对应的SequencerProcessor进行余额更新
        for trade in &trades {
            // 买方账户余额更新
            let buy_shard =
                (trade.buy_account_id % self.sequencer_senders.len() as i32).abs() as usize;
            if let Some(sender) = self.sequencer_senders.get(buy_shard) {
                let trade_msg = TradeExecutionMessage::ExecuteTrade {
                    trade: trade.clone(),
                    original_response_sender: tokio::sync::oneshot::channel().0, // 临时占位
                };
                if let Err(e) = sender.send(trade_msg) {
                    println!("Failed to send trade to sequencer {}: {}", buy_shard, e);
                }
            }

            println!(
                "Trade routed: symbol={}, buy_account={}, sell_account={}, price={}, quantity={}",
                trade.symbol_id,
                trade.buy_account_id,
                trade.sell_account_id,
                trade.price,
                trade.quantity
            );
        }

        // 立即返回撮合成功响应
        let response = crate::models::schema::PlaceOrderResponse {
            code: 0,
            message: Some(format!("Order matched with {} trades", trades.len())),
            id: order_id as i64,
        };
        let _ = response_sender.send(response);
    }

    fn handle_get_order_book(
        &self,
        _request_id: uuid::Uuid,
        symbol_id: i32,
        levels: i32,
        response_sender: tokio::sync::oneshot::Sender<crate::models::schema::GetOrderBookResponse>,
    ) {
        println!(
            "MatchProcessor {}: Getting orderbook for symbol {}, levels {}",
            self.id, symbol_id, levels
        );

        let levels = if levels <= 0 { 20 } else { levels as usize };

        let response = if let Some(order_book) = self.matching_engine.get_order_book(symbol_id) {
            let (bids, asks) = order_book.get_market_depth(levels);

            let bid_levels: Vec<crate::models::schema::PriceLevel> = bids
                .into_iter()
                .map(|(price, quantity)| crate::models::schema::PriceLevel {
                    price: price.to_string(),
                    quantity: quantity.to_string(),
                })
                .collect();

            let ask_levels: Vec<crate::models::schema::PriceLevel> = asks
                .into_iter()
                .map(|(price, quantity)| crate::models::schema::PriceLevel {
                    price: price.to_string(),
                    quantity: quantity.to_string(),
                })
                .collect();

            let best_bid = order_book.get_best_bid().map(|p| p.to_string());
            let best_ask = order_book.get_best_ask().map(|p| p.to_string());
            let spread = order_book.get_spread().map(|s| s.to_string());

            crate::models::schema::GetOrderBookResponse {
                code: 0,
                message: Some("Success".to_string()),
                symbol_id,
                bids: bid_levels,
                asks: ask_levels,
                best_bid,
                best_ask,
                spread,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as i64,
            }
        } else {
            crate::models::schema::GetOrderBookResponse {
                code: 404,
                message: Some("OrderBook not found".to_string()),
                symbol_id,
                bids: vec![],
                asks: vec![],
                best_bid: None,
                best_ask: None,
                spread: None,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as i64,
            }
        };

        let _ = response_sender.send(response);
    }

    fn handle_cancel_order(
        &mut self,
        _request_id: uuid::Uuid,
        symbol_id: i32,
        account_id: i32,
        order_id: u64,
        response_sender: tokio::sync::oneshot::Sender<crate::models::schema::CancelOrderResponse>,
    ) {
        println!(
            "MatchProcessor {}: Cancelling order {} for account {} on symbol {}",
            self.id, order_id, account_id, symbol_id
        );

        let response =
            if let Some(cancelled_order) = self.matching_engine.cancel_order(symbol_id, order_id) {
                // 检查订单是否属于请求的账户
                if cancelled_order.account_id != account_id {
                    crate::models::schema::CancelOrderResponse {
                        code: 403,
                        message: Some("Order does not belong to this account".to_string()),
                        order_id: order_id as i64,
                        cancelled_quantity: None,
                        refund_amount: None,
                    }
                } else {
                    let cancelled_quantity = cancelled_order.remaining_quantity();
                    println!(
                        "MatchProcessor {}: Order {} cancelled, remaining quantity: {}",
                        self.id, order_id, cancelled_quantity
                    );

                    // 发送余额解冻消息到对应的SequencerProcessor
                    let unfreeze_shard =
                        (account_id % self.sequencer_senders.len() as i32).abs() as usize;
                    if let Some(sender) = self.sequencer_senders.get(unfreeze_shard) {
                        let unfreeze_msg = crate::messages::TradeExecutionMessage::UnfreezeOrder {
                            order: cancelled_order.clone(),
                        };
                        if let Err(e) = sender.send(unfreeze_msg) {
                            println!("Failed to send unfreeze message: {}", e);
                        }
                    }

                    crate::models::schema::CancelOrderResponse {
                        code: 0,
                        message: Some("Order cancelled successfully".to_string()),
                        order_id: order_id as i64,
                        cancelled_quantity: Some(cancelled_quantity.to_string()),
                        refund_amount: None, // Will be calculated in SequencerProcessor
                    }
                }
            } else {
                crate::models::schema::CancelOrderResponse {
                    code: 404,
                    message: Some("Order not found".to_string()),
                    order_id: order_id as i64,
                    cancelled_quantity: None,
                    refund_amount: None,
                }
            };

        let _ = response_sender.send(response);
    }
}

impl SequencerProcessor {
    pub fn new(
        id: usize,
        receiver: crossbeam_channel::Receiver<SequencerMessage>,
        match_senders: Vec<crossbeam_channel::Sender<MatchMessage>>,
        trade_execution_receiver: crossbeam_channel::Receiver<TradeExecutionMessage>,
    ) -> Self {
        Self {
            id,
            receiver,
            balance_manager: crate::models::BalanceManager::new(),
            match_senders,
            trade_execution_receiver,
        }
    }

    pub fn run(mut self) {
        println!("SequencerProcessor {} started", self.id);
        loop {
            crossbeam_channel::select! {
                recv(self.receiver) -> message => {
                    match message {
                        Ok(msg) => self.process_sequencer_message(msg),
                        Err(_) => {
                            println!("SequencerProcessor {} stopped - sequencer channel closed", self.id);
                            break;
                        }
                    }
                }
                recv(self.trade_execution_receiver) -> trade_message => {
                    match trade_message {
                        Ok(msg) => self.process_trade_execution_message(msg),
                        Err(_) => {
                            println!("SequencerProcessor {} stopped - trade execution channel closed", self.id);
                            break;
                        }
                    }
                }
            }
        }
    }

    fn process_sequencer_message(&mut self, message: SequencerMessage) {
        match message {
            SequencerMessage::GetAccount {
                request_id: _,
                account_id,
                currency_id,
                response_sender,
            } => {
                let response = self
                    .balance_manager
                    .handle_get_account(account_id, currency_id);
                let _ = response_sender.send(response);
            }
            SequencerMessage::Increase {
                request_id: _,
                account_id,
                currency_id,
                amount,
                response_sender,
            } => {
                let response =
                    self.balance_manager
                        .handle_increase(account_id, currency_id, &amount);
                let _ = response_sender.send(response);
            }
            SequencerMessage::Decrease {
                request_id: _,
                account_id,
                currency_id,
                amount,
                response_sender,
            } => {
                let response =
                    self.balance_manager
                        .handle_decrease(account_id, currency_id, &amount);

                let _ = response_sender.send(response);
            }
            SequencerMessage::PlaceOrder {
                request_id,
                symbol_id,
                account_id,
                order_type,
                side,
                price,
                quantity,
                response_sender,
            } => {
                // 使用新的 handle_place_order 方法来处理订单和冻结余额
                match self
                    .balance_manager
                    .handle_place_order(account_id, symbol_id, side, &price, &quantity)
                {
                    Ok((freeze_currency_id, freeze_amount)) => {
                        println!("Order processed: account_id={}, symbol_id={}, side={}, frozen_currency={}, frozen_amount={}",
                            account_id, symbol_id, side, freeze_currency_id, freeze_amount);

                        // 余额足够，发送到 MatchProcessor
                        let match_message = MatchMessage::PlaceOrder {
                            request_id,
                            symbol_id,
                            account_id,
                            order_type,
                            side,
                            price,
                            quantity,
                            response_sender,
                        };

                        let shard_index =
                            (symbol_id % self.match_senders.len() as i32).abs() as usize;
                        let sender = &self.match_senders[shard_index];

                        if let Err(_) = sender.send(match_message) {
                            println!("Failed to forward to matcher - channel closed");
                            // response_sender is moved to match_message, so we can't send response here
                        }
                    }
                    Err(e) => {
                        // 余额不足或其他错误，返回错误响应
                        let response = crate::models::schema::PlaceOrderResponse {
                            code: 400,
                            message: Some(format!("Order failed: {}", e)),
                            id: 0,
                        };
                        let _ = response_sender.send(response);
                    }
                }
            }
            SequencerMessage::CancelOrder {
                request_id,
                symbol_id,
                account_id,
                order_id,
                response_sender,
            } => {
                // 转发取消订单请求到对应的 MatchProcessor
                let match_message = MatchMessage::CancelOrder {
                    request_id,
                    symbol_id,
                    account_id,
                    order_id,
                    response_sender,
                };

                let shard_index = (symbol_id % self.match_senders.len() as i32).abs() as usize;
                let sender = &self.match_senders[shard_index];

                if let Err(_) = sender.send(match_message) {
                    println!("Failed to forward cancel order to matcher - channel closed");
                    // response_sender was moved to match_message, so we can't send response here
                }
            }
        }
    }

    fn process_trade_execution_message(&mut self, message: TradeExecutionMessage) {
        match message {
            TradeExecutionMessage::ExecuteTrade {
                trade,
                original_response_sender: _,
            } => {
                if let Err(e) = self.execute_single_trade(&trade) {
                    println!(
                        "SequencerProcessor {}: Failed to execute trade {}: {}",
                        self.id, trade.id, e
                    );
                }
            }
            TradeExecutionMessage::UnfreezeOrder { order } => {
                if let Err(e) = self.unfreeze_order_balance(&order) {
                    println!(
                        "SequencerProcessor {}: Failed to unfreeze order {}: {}",
                        self.id, order.id, e
                    );
                }
            }
        }
    }

    fn execute_single_trade(&mut self, trade: &Trade) -> Result<(), BalanceError> {
        // 获取交易对信息
        let symbol = get_symbol(trade.symbol_id).ok_or(BalanceError::CurrencyNotFound)?;

        // 买方：扣除冻结的 quote currency，增加 base currency
        let quote_amount = trade.price * trade.quantity;

        // 处理买方账户（如果属于当前分片）
        let buy_shard = (trade.buy_account_id % 10).abs() as usize; // 假设10个分片
        if buy_shard == self.id {
            let buy_account = self
                .balance_manager
                .accounts
                .entry(trade.buy_account_id)
                .or_insert_with(|| crate::models::Account::new(trade.buy_account_id));

            // 1. 减少冻结的 quote currency
            let buy_quote_balance = buy_account.get_balance(symbol.quote);
            buy_quote_balance.frozen -= quote_amount;
            buy_quote_balance.total -= quote_amount;

            // 2. 增加 base currency
            let buy_base_balance = buy_account.get_balance(symbol.base);
            buy_base_balance.total += trade.quantity;
            buy_base_balance.available += trade.quantity;

            println!(
                "SequencerProcessor {}: Buy account {} - deducted {} {} from frozen, added {} {}",
                self.id,
                trade.buy_account_id,
                quote_amount,
                symbol.quote,
                trade.quantity,
                symbol.base
            );
        }

        // 处理卖方账户（如果属于当前分片）
        let sell_shard = (trade.sell_account_id % 10).abs() as usize;
        if sell_shard == self.id {
            let sell_account = self
                .balance_manager
                .accounts
                .entry(trade.sell_account_id)
                .or_insert_with(|| crate::models::Account::new(trade.sell_account_id));

            // 3. 减少冻结的 base currency
            let sell_base_balance = sell_account.get_balance(symbol.base);
            sell_base_balance.frozen -= trade.quantity;
            sell_base_balance.total -= trade.quantity;

            // 4. 增加 quote currency
            let sell_quote_balance = sell_account.get_balance(symbol.quote);
            sell_quote_balance.total += quote_amount;
            sell_quote_balance.available += quote_amount;

            println!(
                "SequencerProcessor {}: Sell account {} - deducted {} {} from frozen, added {} {}",
                self.id,
                trade.sell_account_id,
                trade.quantity,
                symbol.base,
                quote_amount,
                symbol.quote
            );
        }

        Ok(())
    }

    fn unfreeze_order_balance(
        &mut self,
        order: &crate::matching::Order,
    ) -> Result<(), BalanceError> {
        use crate::matching::OrderSide;
        use crate::models::get_symbol;

        // 获取交易对信息
        let symbol = get_symbol(order.symbol_id).ok_or(BalanceError::CurrencyNotFound)?;

        // 计算需要解冻的金额
        let remaining_quantity = order.remaining_quantity();
        let (unfreeze_currency_id, unfreeze_amount) = match order.side {
            OrderSide::Bid => {
                // 买单：解冻 quote currency
                let quote_amount = order.price * remaining_quantity;
                (symbol.quote, quote_amount)
            }
            OrderSide::Ask => {
                // 卖单：解冻 base currency
                (symbol.base, remaining_quantity)
            }
        };

        // 检查订单是否属于当前分片
        let account_shard = (order.account_id % 10).abs() as usize;
        if account_shard != self.id {
            // 不属于当前分片，不处理
            return Ok(());
        }

        // 解冻余额
        let account = self
            .balance_manager
            .accounts
            .entry(order.account_id)
            .or_insert_with(|| crate::models::Account::new(order.account_id));

        let balance = account.get_balance(unfreeze_currency_id);

        // 检查冻结余额是否足够
        if balance.frozen < unfreeze_amount {
            println!(
                "Warning: Insufficient frozen balance for account {}, currency {}, required: {}, available: {}",
                order.account_id, unfreeze_currency_id, unfreeze_amount, balance.frozen
            );
            // 解冻所有剩余的冻结余额
            let actual_unfreeze = balance.frozen;
            balance.frozen = rust_decimal::Decimal::ZERO;
            balance.available += actual_unfreeze;
        } else {
            // 正常解冻
            balance.frozen -= unfreeze_amount;
            balance.available += unfreeze_amount;
        }

        println!(
            "SequencerProcessor {}: Unfroze {} {} for account {} (order {})",
            self.id, unfreeze_amount, unfreeze_currency_id, order.account_id, order.id
        );

        Ok(())
    }
}
