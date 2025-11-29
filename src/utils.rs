pub struct MessageProcessor {
    id: usize,
    receiver: crossbeam_channel::Receiver<crate::grpc::AsyncBalanceMessage>,
    balance_manager: crate::balance::BalanceManager,
}

impl MessageProcessor {
    pub fn new(
        id: usize,
        receiver: crossbeam_channel::Receiver<crate::grpc::AsyncBalanceMessage>,
    ) -> Self {
        Self {
            id,
            receiver,
            balance_manager: crate::balance::BalanceManager::new(),
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

    fn process_message(&mut self, message: crate::grpc::AsyncBalanceMessage) {
        match message {
            crate::grpc::AsyncBalanceMessage::GetAccount {
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
            crate::grpc::AsyncBalanceMessage::Increase {
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
            crate::grpc::AsyncBalanceMessage::Decrease {
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
        }
    }
}
