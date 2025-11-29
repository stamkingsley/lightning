use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

// 生成的 proto 代码
pub mod schema {
    tonic::include_proto!("schema");
}

use schema::*;

#[derive(Error, Debug)]
pub enum BalanceError {
    #[error("Insufficient balance")]
    InsufficientBalance,
    #[error("Invalid amount: {0}")]
    InvalidAmount(String),
    #[error("Account not found")]
    AccountNotFound,
    #[error("Currency not found")]
    CurrencyNotFound,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBalance {
    pub currency_id: i32,
    pub total: Decimal,
    pub frozen: Decimal,
    pub available: Decimal,
}

impl AccountBalance {
    pub fn new(currency_id: i32) -> Self {
        Self {
            currency_id,
            total: Decimal::ZERO,
            frozen: Decimal::ZERO,
            available: Decimal::ZERO,
        }
    }

    pub fn increase(&mut self, amount: Decimal) -> Result<(), BalanceError> {
        if amount <= Decimal::ZERO {
            return Err(BalanceError::InvalidAmount(
                "Amount must be positive".to_string(),
            ));
        }
        self.total += amount;
        self.available += amount;
        Ok(())
    }

    pub fn decrease(&mut self, amount: Decimal) -> Result<(), BalanceError> {
        if amount <= Decimal::ZERO {
            return Err(BalanceError::InvalidAmount(
                "Amount must be positive".to_string(),
            ));
        }
        if self.available < amount {
            return Err(BalanceError::InsufficientBalance);
        }
        self.total -= amount;
        self.available -= amount;
        Ok(())
    }

    pub fn freeze(&mut self, amount: Decimal) -> Result<(), BalanceError> {
        if amount <= Decimal::ZERO {
            return Err(BalanceError::InvalidAmount(
                "Amount must be positive".to_string(),
            ));
        }
        if self.available < amount {
            return Err(BalanceError::InsufficientBalance);
        }
        self.available -= amount;
        self.frozen += amount;
        Ok(())
    }

    pub fn unfreeze(&mut self, amount: Decimal) -> Result<(), BalanceError> {
        if amount <= Decimal::ZERO {
            return Err(BalanceError::InvalidAmount(
                "Amount must be positive".to_string(),
            ));
        }
        if self.frozen < amount {
            return Err(BalanceError::InsufficientBalance);
        }
        self.frozen -= amount;
        self.available += amount;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Account {
    pub id: i32,
    pub balances: HashMap<i32, AccountBalance>,
}

impl Account {
    pub fn new(id: i32) -> Self {
        Self {
            id,
            balances: HashMap::new(),
        }
    }

    pub fn get_balance(&mut self, currency_id: i32) -> &mut AccountBalance {
        self.balances
            .entry(currency_id)
            .or_insert_with(|| AccountBalance::new(currency_id))
    }
}

// 消息类型定义

// 余额管理器
#[derive(Debug)]
pub struct BalanceManager {
    accounts: HashMap<i32, Account>,
}

impl BalanceManager {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
        }
    }

    pub fn handle_get_account(
        &self,
        account_id: i32,
        currency_id: Option<i32>,
    ) -> GetAccountResponse {
        // 检查账户是否存在
        let account = match self.accounts.get(&account_id) {
            Some(account) => account,
            None => {
                return GetAccountResponse {
                    code: 404,
                    message: Some("Account not found".to_string()),
                    data: HashMap::new(),
                };
            }
        };

        let mut data = HashMap::new();

        match currency_id {
            Some(currency_id) => {
                // 查询特定币种
                if let Some(balance) = account.balances.get(&currency_id) {
                    data.insert(
                        currency_id,
                        Balance {
                            currency: currency_id.to_string(),
                            value: balance.total.to_string(),
                            frozen: balance.frozen.to_string(),
                            available: balance.available.to_string(),
                        },
                    );
                }
            }
            None => {
                // 查询所有币种
                for (&currency_id, balance) in &account.balances {
                    data.insert(
                        currency_id,
                        Balance {
                            currency: currency_id.to_string(),
                            value: balance.total.to_string(),
                            frozen: balance.frozen.to_string(),
                            available: balance.available.to_string(),
                        },
                    );
                }
            }
        }

        GetAccountResponse {
            code: 0,
            message: Some("Success".to_string()),
            data,
        }
    }

    pub fn handle_increase(
        &mut self,
        account_id: i32,
        currency_id: i32,
        amount_str: &str,
    ) -> IncreaseResponse {
        let amount = match Decimal::from_str_exact(amount_str) {
            Ok(amount) => amount,
            Err(_) => {
                return IncreaseResponse {
                    code: 400,
                    message: Some("Invalid amount format".to_string()),
                    data: None,
                };
            }
        };

        let account = self
            .accounts
            .entry(account_id)
            .or_insert_with(|| Account::new(account_id));
        let balance = account.get_balance(currency_id);

        match balance.increase(amount) {
            Ok(_) => {
                let balance_data = Balance {
                    currency: currency_id.to_string(),
                    value: balance.total.to_string(),
                    frozen: balance.frozen.to_string(),
                    available: balance.available.to_string(),
                };
                IncreaseResponse {
                    code: 0,
                    message: Some("Success".to_string()),
                    data: Some(balance_data),
                }
            }
            Err(e) => IncreaseResponse {
                code: 400,
                message: Some(e.to_string()),
                data: None,
            },
        }
    }

    pub fn handle_decrease(
        &mut self,
        account_id: i32,
        currency_id: i32,
        amount_str: &str,
    ) -> DecreaseResponse {
        let amount = match Decimal::from_str_exact(amount_str) {
            Ok(amount) => amount,
            Err(_) => {
                return DecreaseResponse {
                    code: 400,
                    message: Some("Invalid amount format".to_string()),
                    data: None,
                };
            }
        };

        let account = self
            .accounts
            .entry(account_id)
            .or_insert_with(|| Account::new(account_id));
        let balance = account.get_balance(currency_id);

        match balance.decrease(amount) {
            Ok(_) => {
                let balance_data = Balance {
                    currency: currency_id.to_string(),
                    value: balance.total.to_string(),
                    frozen: balance.frozen.to_string(),
                    available: balance.available.to_string(),
                };
                DecreaseResponse {
                    code: 0,
                    message: Some("Success".to_string()),
                    data: Some(balance_data),
                }
            }
            Err(e) => DecreaseResponse {
                code: 400,
                message: Some(e.to_string()),
                data: None,
            },
        }
    }
}
