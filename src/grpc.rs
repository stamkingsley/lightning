use crate::models::schema;
use crossbeam_channel::Sender;
use tokio::sync::oneshot;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::messages::{MatchMessage, SequencerMessage};
use schema::lightning_server::{Lightning, LightningServer};
use schema::{
    CancelOrderRequest, CancelOrderResponse, DecreaseRequest, DecreaseResponse, GetAccountRequest,
    GetAccountResponse, GetOrderBookRequest, GetOrderBookResponse, IncreaseRequest,
    IncreaseResponse,
};

// 使用oneshot channel的异步消息类型
// 使用oneshot channel的异步消息类型

// 高性能异步EnvoyService
pub struct LightningService {
    sequencer_senders: Vec<Sender<SequencerMessage>>,
    match_senders: Vec<Sender<MatchMessage>>,
    shard_count: usize,
}

impl LightningService {
    pub fn new(
        sequencer_senders: Vec<Sender<SequencerMessage>>,
        match_senders: Vec<Sender<MatchMessage>>,
        shard_count: usize,
    ) -> Self {
        Self {
            sequencer_senders,
            match_senders,
            shard_count,
        }
    }
}

#[tonic::async_trait]
impl Lightning for LightningService {
    async fn get_account(
        &self,
        request: Request<GetAccountRequest>,
    ) -> Result<Response<GetAccountResponse>, Status> {
        let req = request.into_inner();
        let request_id = Uuid::new_v4();

        // 使用oneshot channel，开销更小
        let (response_sender, response_receiver) = oneshot::channel();

        let message = SequencerMessage::GetAccount {
            request_id,
            account_id: req.account_id,
            currency_id: req.currency_id,
            response_sender,
        };

        // 计算分片索引
        let shard_index = (req.account_id % self.shard_count as i32).abs() as usize;
        let sender = &self.sequencer_senders[shard_index];

        // 发送消息到 channel
        if let Err(e) = sender.send(message) {
            return Err(Status::internal(format!("Failed to send message: {}", e)));
        }

        // 异步等待响应，不阻塞tokio线程
        match response_receiver.await {
            Ok(response) => Ok(Response::new(response)),
            Err(_) => Err(Status::internal("Failed to receive response")),
        }
    }

    async fn increase(
        &self,
        request: Request<IncreaseRequest>,
    ) -> Result<Response<IncreaseResponse>, Status> {
        let req = request.into_inner();
        let request_id = Uuid::new_v4();

        // 使用oneshot channel
        let (response_sender, response_receiver) = oneshot::channel();

        let message = SequencerMessage::Increase {
            request_id,
            account_id: req.account_id,
            currency_id: req.currency_id,
            amount: req.amount,
            response_sender,
        };

        let shard_index = (req.account_id % self.shard_count as i32).abs() as usize;
        let sender = &self.sequencer_senders[shard_index];

        if let Err(e) = sender.send(message) {
            return Err(Status::internal(format!("Failed to send message: {}", e)));
        }

        // 异步等待响应
        match response_receiver.await {
            Ok(response) => Ok(Response::new(response)),
            Err(_) => Err(Status::internal("Failed to receive response")),
        }
    }

    async fn decrease(
        &self,
        request: Request<DecreaseRequest>,
    ) -> Result<Response<DecreaseResponse>, Status> {
        let req = request.into_inner();
        let request_id = Uuid::new_v4();

        // 使用oneshot channel
        let (response_sender, response_receiver) = oneshot::channel();

        let message = SequencerMessage::Decrease {
            request_id,
            account_id: req.account_id,
            currency_id: req.currency_id,
            amount: req.amount,
            response_sender,
        };

        let shard_index = (req.account_id % self.shard_count as i32).abs() as usize;
        let sender = &self.sequencer_senders[shard_index];

        if let Err(e) = sender.send(message) {
            return Err(Status::internal(format!("Failed to send message: {}", e)));
        }

        // 异步等待响应
        match response_receiver.await {
            Ok(response) => Ok(Response::new(response)),
            Err(_) => Err(Status::internal("Failed to receive response")),
        }
    }

    async fn place_order(
        &self,
        request: Request<schema::PlaceOrderRequest>,
    ) -> Result<Response<schema::PlaceOrderResponse>, Status> {
        let req = request.into_inner();
        let request_id = Uuid::new_v4();

        let (response_sender, response_receiver) = oneshot::channel();

        let message = SequencerMessage::PlaceOrder {
            request_id,
            symbol_id: req.symbol_id,
            account_id: req.account_id,
            order_type: req.r#type,
            side: req.side,
            price: req.price.unwrap_or_default(),
            quantity: req.quantity.unwrap_or_default(),
            response_sender,
        };

        let shard_index = (req.account_id % self.shard_count as i32).abs() as usize;
        let sender = &self.sequencer_senders[shard_index];

        if let Err(e) = sender.send(message) {
            return Err(Status::internal(format!("Failed to send message: {}", e)));
        }

        match response_receiver.await {
            Ok(response) => Ok(Response::new(response)),
            Err(_) => Err(Status::internal("Failed to receive response")),
        }
    }

    async fn get_order_book(
        &self,
        request: Request<GetOrderBookRequest>,
    ) -> Result<Response<GetOrderBookResponse>, Status> {
        let req = request.into_inner();
        let request_id = Uuid::new_v4();

        let (response_sender, response_receiver) = oneshot::channel();

        let message = MatchMessage::GetOrderBook {
            request_id,
            symbol_id: req.symbol_id,
            levels: req.levels.unwrap_or(20),
            response_sender,
        };

        // 路由到对应的 MatchProcessor (按symbol_id分片)
        let shard_index = (req.symbol_id % self.shard_count as i32).abs() as usize;
        let sender = &self.match_senders[shard_index];

        if let Err(e) = sender.send(message) {
            return Err(Status::internal(format!("Failed to send message: {}", e)));
        }

        match response_receiver.await {
            Ok(response) => Ok(Response::new(response)),
            Err(_) => Err(Status::internal("Failed to receive response")),
        }
    }

    async fn cancel_order(
        &self,
        request: Request<CancelOrderRequest>,
    ) -> Result<Response<CancelOrderResponse>, Status> {
        let req = request.into_inner();
        let request_id = Uuid::new_v4();

        let (response_sender, response_receiver) = oneshot::channel();

        let message = SequencerMessage::CancelOrder {
            request_id,
            symbol_id: req.symbol_id,
            account_id: req.account_id,
            order_id: req.order_id as u64,
            response_sender,
        };

        // 路由到对应的 SequencerProcessor (按account_id分片)
        let shard_index = (req.account_id % self.shard_count as i32).abs() as usize;
        let sender = &self.sequencer_senders[shard_index];

        if let Err(e) = sender.send(message) {
            return Err(Status::internal(format!("Failed to send message: {}", e)));
        }

        match response_receiver.await {
            Ok(response) => Ok(Response::new(response)),
            Err(_) => Err(Status::internal("Failed to receive response")),
        }
    }
}

pub fn create_server(
    sequencer_senders: Vec<Sender<SequencerMessage>>,
    match_senders: Vec<Sender<MatchMessage>>,
    shard_count: usize,
) -> LightningServer<LightningService> {
    let service = LightningService::new(sequencer_senders, match_senders, shard_count);
    LightningServer::new(service)
}
