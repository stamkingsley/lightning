use crate::matching::Trade;
use crate::models::schema;
use tokio::sync::oneshot;
use uuid::Uuid;

// 使用oneshot channel的异步消息类型
#[derive(Debug)]
pub enum SequencerMessage {
    GetAccount {
        request_id: Uuid,
        account_id: i32,
        currency_id: Option<i32>,
        response_sender: oneshot::Sender<schema::GetAccountResponse>,
    },
    Increase {
        request_id: Uuid,
        account_id: i32,
        currency_id: i32,
        amount: String,
        response_sender: oneshot::Sender<schema::IncreaseResponse>,
    },
    Decrease {
        request_id: Uuid,
        account_id: i32,
        currency_id: i32,
        amount: String,
        response_sender: oneshot::Sender<schema::DecreaseResponse>,
    },
    PlaceOrder {
        request_id: Uuid,
        symbol_id: i32,
        account_id: i32,
        order_type: i32,
        side: i32,
        price: String,
        quantity: String,
        response_sender: oneshot::Sender<schema::PlaceOrderResponse>,
    },
    CancelOrder {
        request_id: Uuid,
        symbol_id: i32,
        account_id: i32,
        order_id: u64,
        response_sender: oneshot::Sender<schema::CancelOrderResponse>,
    },
}

#[derive(Debug)]
pub enum MatchMessage {
    PlaceOrder {
        request_id: Uuid,
        symbol_id: i32,
        account_id: i32,
        order_type: i32,
        side: i32,
        price: String,
        quantity: String,
        response_sender: oneshot::Sender<schema::PlaceOrderResponse>,
    },
    GetOrderBook {
        request_id: Uuid,
        symbol_id: i32,
        levels: i32,
        response_sender: oneshot::Sender<schema::GetOrderBookResponse>,
    },
    CancelOrder {
        request_id: Uuid,
        symbol_id: i32,
        account_id: i32,
        order_id: u64,
        response_sender: oneshot::Sender<schema::CancelOrderResponse>,
    },
}

// 新增：成交执行消息，用于从撮合引擎回调到SequencerProcessor
#[derive(Debug)]
pub enum TradeExecutionMessage {
    ExecuteTrade {
        trade: Trade,
        original_response_sender: oneshot::Sender<schema::PlaceOrderResponse>,
    },
    // 单个账户结算消息：包含该账户的余额变更
    SettleAccount {
        account_id: i32,
        symbol_id: i32,
        deduct_currency_id: i32,  // 需要扣除的币种ID（从冻结余额扣除）
        deduct_amount: rust_decimal::Decimal,  // 需要扣除的数量
        add_currency_id: i32,      // 需要增加的币种ID（增加到可用余额）
        add_amount: rust_decimal::Decimal,      // 需要增加的数量
    },
    UnfreezeOrder {
        order: crate::matching::Order,
    },
}
