use crate::messages::{MatchMessage, SequencerMessage};

pub struct MessageProcessor {
    id: usize,
    receiver: crossbeam_channel::Receiver<SequencerMessage>,
    balance_manager: crate::balance::BalanceManager,
    match_senders: Vec<crossbeam_channel::Sender<MatchMessage>>,
}

pub struct MatchProcessor {
    id: usize,
    receiver: crossbeam_channel::Receiver<MatchMessage>,
}

impl MatchProcessor {
    pub fn new(id: usize, receiver: crossbeam_channel::Receiver<MatchMessage>) -> Self {
        Self { id, receiver }
    }

    pub fn run(self) {
        println!("Match processor {} started", self.id);
        loop {
            match self.receiver.recv() {
                Ok(message) => match message {
                    MatchMessage::PlaceOrder {
                        request_id: _,
                        symbol_id: _,
                        account_id: _,
                        order_type: _,
                        side: _,
                        price: _,
                        quantity: _,
                        response_sender,
                    } => {
                        // TODO: Implement matching logic
                        // For now just return success
                        let response = crate::balance::schema::PlaceOrderResponse {
                            code: 0,
                            message: Some("Order matched".to_string()),
                            id: 1, // Mock order ID
                        };
                        let _ = response_sender.send(response);
                    }
                },
                Err(_) => {
                    println!("Match channel closed");
                    break;
                }
            }
        }
    }
}

impl MessageProcessor {
    pub fn new(
        id: usize,
        receiver: crossbeam_channel::Receiver<SequencerMessage>,
        match_senders: Vec<crossbeam_channel::Sender<MatchMessage>>,
    ) -> Self {
        Self {
            id,
            receiver,
            balance_manager: crate::balance::BalanceManager::new(),
            match_senders,
        }
    }

    pub fn run(mut self) {
        println!("High performance message processor {} started", self.id);
        loop {
            match self.receiver.recv() {
                Ok(message) => {
                    self.process_message(message);
                }
                Err(_) => {
                    println!("Channel closed, stopping high performance processor");
                    break;
                }
            }
        }
    }

    fn process_message(&mut self, message: SequencerMessage) {
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
                // 1. Check/Freeze balance (TODO: Implement freeze logic)
                // For now assuming balance is sufficient

                // 2. Forward to MatchProcessor
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

                let shard_index = (symbol_id % self.match_senders.len() as i32).abs() as usize;
                let sender = &self.match_senders[shard_index];

                if let Err(e) = sender.send(match_message) {
                    println!("Failed to forward to matcher: {}", e);
                    // TODO: Send error response
                }
            }
        }
    }
}
