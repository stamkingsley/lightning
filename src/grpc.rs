use crate::balance::schema;
use crossbeam_channel::Sender;
use tokio::sync::oneshot;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use schema::lightning_server::{Lightning, LightningServer};
use schema::{
    DecreaseRequest, DecreaseResponse, GetAccountRequest, GetAccountResponse, IncreaseRequest,
    IncreaseResponse,
};

// 使用oneshot channel的异步消息类型
#[derive(Debug)]
pub enum AsyncBalanceMessage {
    GetAccount {
        request_id: Uuid,
        account_id: i32,
        currency_id: Option<i32>,
        response_sender: oneshot::Sender<GetAccountResponse>,
    },
    Increase {
        request_id: Uuid,
        account_id: i32,
        currency_id: i32,
        amount: String,
        response_sender: oneshot::Sender<IncreaseResponse>,
    },
    Decrease {
        request_id: Uuid,
        account_id: i32,
        currency_id: i32,
        amount: String,
        response_sender: oneshot::Sender<DecreaseResponse>,
    },
}

// 高性能异步EnvoyService
pub struct LightningService {
    message_senders: Vec<Sender<AsyncBalanceMessage>>,
    shard_count: usize,
}

impl LightningService {
    pub fn new(message_senders: Vec<Sender<AsyncBalanceMessage>>, shard_count: usize) -> Self {
        Self {
            message_senders,
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

        let message = AsyncBalanceMessage::GetAccount {
            request_id,
            account_id: req.account_id,
            currency_id: req.currency_id,
            response_sender,
        };

        // 计算分片索引
        let shard_index = (req.account_id % self.shard_count as i32).abs() as usize;
        let sender = &self.message_senders[shard_index];

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

        let message = AsyncBalanceMessage::Increase {
            request_id,
            account_id: req.account_id,
            currency_id: req.currency_id,
            amount: req.amount,
            response_sender,
        };

        let shard_index = (req.account_id % self.shard_count as i32).abs() as usize;
        let sender = &self.message_senders[shard_index];

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

        let message = AsyncBalanceMessage::Decrease {
            request_id,
            account_id: req.account_id,
            currency_id: req.currency_id,
            amount: req.amount,
            response_sender,
        };

        let shard_index = (req.account_id % self.shard_count as i32).abs() as usize;
        let sender = &self.message_senders[shard_index];

        if let Err(e) = sender.send(message) {
            return Err(Status::internal(format!("Failed to send message: {}", e)));
        }

        // 异步等待响应
        match response_receiver.await {
            Ok(response) => Ok(Response::new(response)),
            Err(_) => Err(Status::internal("Failed to receive response")),
        }
    }
}

pub fn create_server(
    message_senders: Vec<Sender<AsyncBalanceMessage>>,
    shard_count: usize,
) -> LightningServer<LightningService> {
    let service = LightningService::new(message_senders, shard_count);
    LightningServer::new(service)
}
