use crate::balance::schema;
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
}
