use lightning::{
    messages::{MatchMessage, SequencerMessage},
    models::{init_global_config, BalanceManager},
};
use std::sync::mpsc;
use tokio::sync::oneshot;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    println!("=== Lightning Order Processing Demo ===\n");

    // 初始化全局配置
    init_global_config();
    println!("✓ Global currencies and symbols initialized");
    println!("  - BTC (id: 1)");
    println!("  - USDT (id: 2)");
    println!("  - BTC-USDT (symbol_id: 1, base: BTC, quote: USDT)\n");

    // 创建余额管理器
    let mut balance_manager = BalanceManager::new();

    // 示例账户ID
    let account_id = 1001;
    let symbol_id = 1; // BTC-USDT

    println!("=== 初始化账户余额 ===");

    // 给账户充值 BTC 和 USDT
    let btc_response = balance_manager.handle_increase(account_id, 1, "2.0"); // 2 BTC
    println!("充值 2.0 BTC: {:?}", btc_response.message);

    let usdt_response = balance_manager.handle_increase(account_id, 2, "100000.0"); // 100,000 USDT
    println!("充值 100,000 USDT: {:?}", usdt_response.message);

    // 查看初始余额
    println!("\n=== 初始余额 ===");
    let account_response = balance_manager.handle_get_account(account_id, None);
    for (currency_id, balance) in &account_response.data {
        let currency_name = if *currency_id == 1 { "BTC" } else { "USDT" };
        println!(
            "{}: total={}, available={}, frozen={}",
            currency_name, balance.value, balance.available, balance.frozen
        );
    }

    println!("\n=== 订单处理演示 ===");

    // 演示1: 买入订单 (BID) - 用 USDT 买 BTC
    println!("\n1. 买入订单 (BID): 用 50,000 USDT 买 1.0 BTC");
    println!("   - 价格: 50,000 USDT");
    println!("   - 数量: 1.0 BTC");
    println!("   - 需要冻结: 50,000 * 1.0 = 50,000 USDT");

    match balance_manager.handle_place_order(account_id, symbol_id, 0, "50000.0", "1.0") {
        Ok((frozen_currency, frozen_amount)) => {
            let currency_name = if frozen_currency == 1 { "BTC" } else { "USDT" };
            println!("   ✓ 订单处理成功");
            println!("   ✓ 冻结货币: {} (id: {})", currency_name, frozen_currency);
            println!("   ✓ 冻结金额: {}", frozen_amount);
        }
        Err(e) => {
            println!("   ✗ 订单失败: {}", e);
        }
    }

    // 查看买入订单后的余额
    println!("\n   买入订单后的余额:");
    let account_response = balance_manager.handle_get_account(account_id, None);
    for (currency_id, balance) in &account_response.data {
        let currency_name = if *currency_id == 1 { "BTC" } else { "USDT" };
        println!(
            "   {}: total={}, available={}, frozen={}",
            currency_name, balance.value, balance.available, balance.frozen
        );
    }

    // 演示2: 卖出订单 (ASK) - 卖 BTC 换 USDT
    println!("\n2. 卖出订单 (ASK): 卖 0.5 BTC，价格 55,000 USDT");
    println!("   - 价格: 55,000 USDT");
    println!("   - 数量: 0.5 BTC");
    println!("   - 需要冻结: 0.5 BTC");

    match balance_manager.handle_place_order(account_id, symbol_id, 1, "55000.0", "0.5") {
        Ok((frozen_currency, frozen_amount)) => {
            let currency_name = if frozen_currency == 1 { "BTC" } else { "USDT" };
            println!("   ✓ 订单处理成功");
            println!("   ✓ 冻结货币: {} (id: {})", currency_name, frozen_currency);
            println!("   ✓ 冻结金额: {}", frozen_amount);
        }
        Err(e) => {
            println!("   ✗ 订单失败: {}", e);
        }
    }

    // 查看卖出订单后的余额
    println!("\n   卖出订单后的余额:");
    let account_response = balance_manager.handle_get_account(account_id, None);
    for (currency_id, balance) in &account_response.data {
        let currency_name = if *currency_id == 1 { "BTC" } else { "USDT" };
        println!(
            "   {}: total={}, available={}, frozen={}",
            currency_name, balance.value, balance.available, balance.frozen
        );
    }

    // 演示3: 余额不足的订单
    println!("\n3. 余额不足的订单测试: 尝试卖出 5.0 BTC (超过可用余额)");
    match balance_manager.handle_place_order(account_id, symbol_id, 1, "60000.0", "5.0") {
        Ok((frozen_currency, frozen_amount)) => {
            println!("   意外成功 - 这不应该发生");
        }
        Err(e) => {
            println!("   ✓ 正确拒绝订单: {}", e);
        }
    }

    // 演示4: 无效交易对的订单
    println!("\n4. 无效交易对测试: 使用不存在的交易对 (id: 999)");
    match balance_manager.handle_place_order(account_id, 999, 0, "50000.0", "1.0") {
        Ok((frozen_currency, frozen_amount)) => {
            println!("   意外成功 - 这不应该发生");
        }
        Err(e) => {
            println!("   ✓ 正确拒绝订单: {}", e);
        }
    }

    println!("\n=== 最终余额状态 ===");
    let final_response = balance_manager.handle_get_account(account_id, None);
    for (currency_id, balance) in &final_response.data {
        let currency_name = if *currency_id == 1 { "BTC" } else { "USDT" };
        println!(
            "{}: total={}, available={}, frozen={}",
            currency_name, balance.value, balance.available, balance.frozen
        );
    }

    println!("\n=== 订单处理逻辑总结 ===");
    println!("1. 买入订单 (BID, side=0):");
    println!("   - 冻结 quote currency (USDT)");
    println!("   - 冻结金额 = 价格 × 数量");
    println!("\n2. 卖出订单 (ASK, side=1):");
    println!("   - 冻结 base currency (BTC)");
    println!("   - 冻结金额 = 数量");
    println!("\n3. 风控检查:");
    println!("   - 检查余额是否充足");
    println!("   - 验证交易对是否存在");
    println!("   - 验证价格和数量格式");

    println!("\n=== 演示完成 ===");
}
