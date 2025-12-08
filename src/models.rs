use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;
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
pub struct Currency {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub id: i32,
    pub name: String,
    pub base: i32,  // base currency id
    pub quote: i32, // quote currency id
}

// 全局符号配置
static GLOBAL_SYMBOLS: OnceLock<HashMap<i32, Symbol>> = OnceLock::new();
static GLOBAL_CURRENCIES: OnceLock<HashMap<i32, Currency>> = OnceLock::new();

pub fn init_global_config() {
    // 初始化货币
    GLOBAL_CURRENCIES.get_or_init(|| {
        let mut currencies = HashMap::new();
        currencies.insert(
            1,
            Currency {
                id: 1,
                name: "BTC".to_string(),
            },
        );
        currencies.insert(
            2,
            Currency {
                id: 2,
                name: "USDT".to_string(),
            },
        );
        currencies
    });

    // 初始化交易对
    GLOBAL_SYMBOLS.get_or_init(|| {
        let mut symbols = HashMap::new();
        symbols.insert(
            1,
            Symbol {
                id: 1,
                name: "BTC-USDT".to_string(),
                base: 1,  // BTC
                quote: 2, // USDT
            },
        );
        symbols
    });
}

pub fn get_symbol(symbol_id: i32) -> Option<&'static Symbol> {
    GLOBAL_SYMBOLS.get()?.get(&symbol_id)
}

pub fn get_currency(currency_id: i32) -> Option<&'static Currency> {
    GLOBAL_CURRENCIES.get()?.get(&currency_id)
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
    pub accounts: HashMap<i32, Account>,
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

    pub fn handle_freeze(
        &mut self,
        account_id: i32,
        currency_id: i32,
        amount_str: &str,
    ) -> Result<(), BalanceError> {
        let amount = match Decimal::from_str_exact(amount_str) {
            Ok(amount) => amount,
            Err(_) => {
                return Err(BalanceError::InvalidAmount(
                    "Invalid amount format".to_string(),
                ));
            }
        };

        let account = self
            .accounts
            .entry(account_id)
            .or_insert_with(|| Account::new(account_id));
        let balance = account.get_balance(currency_id);

        balance.freeze(amount)
    }

    pub fn handle_place_order(
        &mut self,
        account_id: i32,
        symbol_id: i32,
        side: i32,
        price: &str,
        quantity: &str,
    ) -> Result<(i32, String), BalanceError> {
        // 获取交易对信息
        let symbol = get_symbol(symbol_id).ok_or(BalanceError::CurrencyNotFound)?;

        let (freeze_currency_id, freeze_amount) = if side == 0 {
            // BID (买入): 冻结 quote currency，金额 = price * quantity
            let price_decimal = Decimal::from_str_exact(price)
                .map_err(|_| BalanceError::InvalidAmount("Invalid price format".to_string()))?;
            let quantity_decimal = Decimal::from_str_exact(quantity)
                .map_err(|_| BalanceError::InvalidAmount("Invalid quantity format".to_string()))?;
            let freeze_amount = price_decimal * quantity_decimal;
            (symbol.quote, freeze_amount)
        } else {
            // ASK (卖出): 冻结 base currency，金额 = quantity
            let quantity_decimal = Decimal::from_str_exact(quantity)
                .map_err(|_| BalanceError::InvalidAmount("Invalid quantity format".to_string()))?;
            (symbol.base, quantity_decimal)
        };

        // 尝试冻结余额
        self.handle_freeze(account_id, freeze_currency_id, &freeze_amount.to_string())?;

        Ok((freeze_currency_id, freeze_amount.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn ensure_global_config() {
        INIT.call_once(|| {
            init_global_config();
        });
    }

    #[test]
    fn test_currency_initialization() {
        ensure_global_config();

        let btc = get_currency(1).unwrap();
        assert_eq!(btc.id, 1);
        assert_eq!(btc.name, "BTC");

        let usdt = get_currency(2).unwrap();
        assert_eq!(usdt.id, 2);
        assert_eq!(usdt.name, "USDT");
    }

    #[test]
    fn test_symbol_initialization() {
        ensure_global_config();

        let btc_usdt = get_symbol(1).unwrap();
        assert_eq!(btc_usdt.id, 1);
        assert_eq!(btc_usdt.name, "BTC-USDT");
        assert_eq!(btc_usdt.base, 1); // BTC
        assert_eq!(btc_usdt.quote, 2); // USDT
    }

    #[test]
    fn test_balance_operations() {
        let mut balance = AccountBalance::new(1);

        // Test increase
        assert!(balance.increase(Decimal::new(100, 0)).is_ok());
        assert_eq!(balance.total, Decimal::new(100, 0));
        assert_eq!(balance.available, Decimal::new(100, 0));
        assert_eq!(balance.frozen, Decimal::ZERO);

        // Test freeze
        assert!(balance.freeze(Decimal::new(30, 0)).is_ok());
        assert_eq!(balance.total, Decimal::new(100, 0));
        assert_eq!(balance.available, Decimal::new(70, 0));
        assert_eq!(balance.frozen, Decimal::new(30, 0));

        // Test insufficient balance for freeze
        assert!(balance.freeze(Decimal::new(80, 0)).is_err());

        // Test unfreeze
        assert!(balance.unfreeze(Decimal::new(10, 0)).is_ok());
        assert_eq!(balance.total, Decimal::new(100, 0));
        assert_eq!(balance.available, Decimal::new(80, 0));
        assert_eq!(balance.frozen, Decimal::new(20, 0));
    }

    #[test]
    fn test_bid_order_processing() {
        ensure_global_config();
        let mut manager = BalanceManager::new();

        // 先给账户充值 USDT (quote currency)
        let _ = manager.handle_increase(1, 2, "1000.0");

        // 测试买入订单 (BID): 应该冻结 USDT
        let result = manager.handle_place_order(1, 1, 0, "50000.0", "0.01");
        assert!(result.is_ok());

        let (frozen_currency, frozen_amount) = result.unwrap();
        assert_eq!(frozen_currency, 2); // USDT
        assert_eq!(frozen_amount, "500.000"); // 50000 * 0.01 = 500

        // 检查余额
        let account_response = manager.handle_get_account(1, Some(2));
        let usdt_balance = account_response.data.get(&2).unwrap();

        // 使用 Decimal 比较而不是字符串比较
        let available = Decimal::from_str_exact(&usdt_balance.available).unwrap();
        let frozen = Decimal::from_str_exact(&usdt_balance.frozen).unwrap();
        let total = Decimal::from_str_exact(&usdt_balance.value).unwrap();

        assert_eq!(available, Decimal::new(500, 0));
        assert_eq!(frozen, Decimal::new(500, 0));
        assert_eq!(total, Decimal::new(1000, 0));
    }

    #[test]
    fn test_ask_order_processing() {
        ensure_global_config();
        let mut manager = BalanceManager::new();

        // 先给账户充值 BTC (base currency)
        let _ = manager.handle_increase(1, 1, "1.0");

        // 测试卖出订单 (ASK): 应该冻结 BTC
        let result = manager.handle_place_order(1, 1, 1, "50000.0", "0.5");
        assert!(result.is_ok());

        let (frozen_currency, frozen_amount) = result.unwrap();
        assert_eq!(frozen_currency, 1); // BTC
        assert_eq!(frozen_amount, "0.5"); // quantity

        // 检查余额
        let account_response = manager.handle_get_account(1, Some(1));
        let btc_balance = account_response.data.get(&1).unwrap();
        assert_eq!(btc_balance.available, "0.5");
        assert_eq!(btc_balance.frozen, "0.5");
        assert_eq!(btc_balance.value, "1.0");
    }

    #[test]
    fn test_insufficient_balance_order() {
        ensure_global_config();
        let mut manager = BalanceManager::new();

        // 不给账户充值，直接下单
        let result = manager.handle_place_order(1, 1, 0, "50000.0", "0.01");
        assert!(result.is_err());

        match result {
            Err(BalanceError::InsufficientBalance) => {}
            _ => panic!("Expected InsufficientBalance error"),
        }
    }

    #[test]
    fn test_invalid_symbol_order() {
        ensure_global_config();
        let mut manager = BalanceManager::new();

        // 使用不存在的交易对
        let result = manager.handle_place_order(1, 999, 0, "50000.0", "0.01");
        assert!(result.is_err());

        match result {
            Err(BalanceError::CurrencyNotFound) => {}
            _ => panic!("Expected CurrencyNotFound error"),
        }
    }
}
