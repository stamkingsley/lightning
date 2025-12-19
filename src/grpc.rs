use crate::models::{schema, ManagementManager};
use crossbeam_channel::Sender;
use std::sync::Arc;
use tokio::sync::oneshot;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::messages::{MatchMessage, SequencerMessage};
use schema::lightning_server::{Lightning, LightningServer};
use schema::management_server::{Management, ManagementServer};
use schema::{
    CancelOrderRequest, CancelOrderResponse, CreateCurrencyRequest, CreateCurrencyResponse,
    CreateSymbolRequest, CreateSymbolResponse, DecreaseRequest, DecreaseResponse,
    DeleteCurrencyRequest, DeleteCurrencyResponse, DeleteSymbolRequest, DeleteSymbolResponse,
    GetAccountRequest, GetAccountResponse, GetCurrencyRequest, GetCurrencyResponse,
    GetOrderBookRequest, GetOrderBookResponse, GetSymbolRequest, GetSymbolResponse,
    IncreaseRequest, IncreaseResponse, ListCurrenciesRequest, ListCurrenciesResponse,
    ListSymbolsRequest, ListSymbolsResponse, UpdateCurrencyRequest, UpdateCurrencyResponse,
    UpdateSymbolRequest, UpdateSymbolResponse,
};


pub struct LightningService {
    sequencer_senders: Vec<Sender<SequencerMessage>>,
    match_senders: Vec<Sender<MatchMessage>>,
    shard_count: usize,
    management_manager: ManagementManager,
}

impl LightningService {
    pub fn new(
        sequencer_senders: Vec<Sender<SequencerMessage>>,
        match_senders: Vec<Sender<MatchMessage>>,
        shard_count: usize,
        management_manager: ManagementManager,
    ) -> Self {
        Self {
            sequencer_senders,
            match_senders,
            shard_count,
            management_manager,
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

#[tonic::async_trait]
impl Management for LightningService {
    async fn create_currency(
        &self,
        request: Request<CreateCurrencyRequest>,
    ) -> Result<Response<CreateCurrencyResponse>, Status> {
        let req = request.into_inner();
        let currency = self.management_manager.create_currency(req.name, req.display_name);

        Ok(Response::new(CreateCurrencyResponse {
            code: 0,
            message: Some("Success".to_string()),
            data: Some(schema::Currency {
                id: currency.id,
                name: currency.name,
                display_name: currency.display_name,
            }),
        }))
    }

    async fn get_currency(
        &self,
        request: Request<GetCurrencyRequest>,
    ) -> Result<Response<GetCurrencyResponse>, Status> {
        let req = request.into_inner();
        match self.management_manager.get_currency(req.id) {
            Some(currency) => Ok(Response::new(GetCurrencyResponse {
                code: 0,
                message: Some("Success".to_string()),
                data: Some(schema::Currency {
                    id: currency.id,
                    name: currency.name,
                    display_name: currency.display_name,
                }),
            })),
            None => Ok(Response::new(GetCurrencyResponse {
                code: 404,
                message: Some("Currency not found".to_string()),
                data: None,
            })),
        }
    }

    async fn list_currencies(
        &self,
        request: Request<ListCurrenciesRequest>,
    ) -> Result<Response<ListCurrenciesResponse>, Status> {
        let req = request.into_inner();
        let currencies = self.management_manager.list_currencies(req.page, req.page_size);
        let total = currencies.len() as i32;

        let data: Vec<schema::Currency> = currencies
            .into_iter()
            .map(|c| schema::Currency {
                id: c.id,
                name: c.name,
                display_name: c.display_name,
            })
            .collect();

        Ok(Response::new(ListCurrenciesResponse {
            code: 0,
            message: Some("Success".to_string()),
            data,
            total: Some(total),
        }))
    }

    async fn update_currency(
        &self,
        request: Request<UpdateCurrencyRequest>,
    ) -> Result<Response<UpdateCurrencyResponse>, Status> {
        let req = request.into_inner();
        match self.management_manager.update_currency(req.id, req.name, req.display_name) {
            Some(currency) => Ok(Response::new(UpdateCurrencyResponse {
                code: 0,
                message: Some("Success".to_string()),
                data: Some(schema::Currency {
                    id: currency.id,
                    name: currency.name,
                    display_name: currency.display_name,
                }),
            })),
            None => Ok(Response::new(UpdateCurrencyResponse {
                code: 404,
                message: Some("Currency not found".to_string()),
                data: None,
            })),
        }
    }

    async fn delete_currency(
        &self,
        request: Request<DeleteCurrencyRequest>,
    ) -> Result<Response<DeleteCurrencyResponse>, Status> {
        let req = request.into_inner();
        if self.management_manager.delete_currency(req.id) {
            Ok(Response::new(DeleteCurrencyResponse {
                code: 0,
                message: Some("Success".to_string()),
            }))
        } else {
            Ok(Response::new(DeleteCurrencyResponse {
                code: 404,
                message: Some("Currency not found".to_string()),
            }))
        }
    }

    async fn create_symbol(
        &self,
        request: Request<CreateSymbolRequest>,
    ) -> Result<Response<CreateSymbolResponse>, Status> {
        let req = request.into_inner();
        match self.management_manager.create_symbol(req.name, req.base, req.quote) {
            Ok(symbol) => Ok(Response::new(CreateSymbolResponse {
                code: 0,
                message: Some("Success".to_string()),
                data: Some(schema::Symbol {
                    id: symbol.id,
                    name: symbol.name,
                    base: symbol.base,
                    quote: symbol.quote,
                }),
            })),
            Err(_) => Ok(Response::new(CreateSymbolResponse {
                code: 400,
                message: Some("Invalid base or quote currency".to_string()),
                data: None,
            })),
        }
    }

    async fn get_symbol(
        &self,
        request: Request<GetSymbolRequest>,
    ) -> Result<Response<GetSymbolResponse>, Status> {
        let req = request.into_inner();
        match self.management_manager.get_symbol(req.id) {
            Some(symbol) => Ok(Response::new(GetSymbolResponse {
                code: 0,
                message: Some("Success".to_string()),
                data: Some(schema::Symbol {
                    id: symbol.id,
                    name: symbol.name,
                    base: symbol.base,
                    quote: symbol.quote,
                }),
            })),
            None => Ok(Response::new(GetSymbolResponse {
                code: 404,
                message: Some("Symbol not found".to_string()),
                data: None,
            })),
        }
    }

    async fn list_symbols(
        &self,
        request: Request<ListSymbolsRequest>,
    ) -> Result<Response<ListSymbolsResponse>, Status> {
        let req = request.into_inner();
        let symbols = self.management_manager.list_symbols(req.page, req.page_size);
        let total = symbols.len() as i32;

        let data: Vec<schema::Symbol> = symbols
            .into_iter()
            .map(|s| schema::Symbol {
                id: s.id,
                name: s.name,
                base: s.base,
                quote: s.quote,
            })
            .collect();

        Ok(Response::new(ListSymbolsResponse {
            code: 0,
            message: Some("Success".to_string()),
            data,
            total: Some(total),
        }))
    }

    async fn update_symbol(
        &self,
        request: Request<UpdateSymbolRequest>,
    ) -> Result<Response<UpdateSymbolResponse>, Status> {
        let req = request.into_inner();
        match self.management_manager.update_symbol(req.id, req.name, req.base, req.quote) {
            Some(symbol) => Ok(Response::new(UpdateSymbolResponse {
                code: 0,
                message: Some("Success".to_string()),
                data: Some(schema::Symbol {
                    id: symbol.id,
                    name: symbol.name,
                    base: symbol.base,
                    quote: symbol.quote,
                }),
            })),
            None => Ok(Response::new(UpdateSymbolResponse {
                code: 404,
                message: Some("Symbol not found".to_string()),
                data: None,
            })),
        }
    }

    async fn delete_symbol(
        &self,
        request: Request<DeleteSymbolRequest>,
    ) -> Result<Response<DeleteSymbolResponse>, Status> {
        let req = request.into_inner();
        if self.management_manager.delete_symbol(req.id) {
            Ok(Response::new(DeleteSymbolResponse {
                code: 0,
                message: Some("Success".to_string()),
            }))
        } else {
            Ok(Response::new(DeleteSymbolResponse {
                code: 404,
                message: Some("Symbol not found".to_string()),
            }))
        }
    }
}

pub fn create_server(
    sequencer_senders: Vec<Sender<SequencerMessage>>,
    match_senders: Vec<Sender<MatchMessage>>,
    shard_count: usize,
    management_manager: ManagementManager,
) -> (LightningServer<LightningService>, ManagementServer<LightningService>) {
    let service1 = LightningService::new(
        sequencer_senders.clone(),
        match_senders.clone(),
        shard_count,
        management_manager.clone(),
    );
    let service2 = LightningService::new(
        sequencer_senders,
        match_senders,
        shard_count,
        management_manager,
    );
    (
        LightningServer::new(service1),
        ManagementServer::new(service2),
    )
}
