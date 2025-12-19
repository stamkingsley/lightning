use lightning::models::schema::lightning_client::LightningClient;
use lightning::models::schema::{
    GetAccountRequest, GetOrderBookRequest, IncreaseRequest, PlaceOrderRequest, Side, Type,
};
use std::time::Duration;
use tokio::time::sleep;
use tonic::Request;

// 常量定义
const ACCOUNT_A: i32 = 1;
const ACCOUNT_B: i32 = 2;
const BTC_CURRENCY_ID: i32 = 1;  // BTC
const USDT_CURRENCY_ID: i32 = 2; // USDT
const SYMBOL_ID: i32 = 1;        // BTC-USDT

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Lightning Integration Test ===");
    
    // 连接到 gRPC 服务
    let mut client = LightningClient::connect("http://127.0.0.1:50051").await?;
    println!("✓ Connected to gRPC server");

    // 1. 创建用户 a，增加 USDT (用于买 BTC)
    println!("\n--- Step 1: Create user A and add USDT ---");
    let increase_a_request = Request::new(IncreaseRequest {
        request_id: 1,
        account_id: ACCOUNT_A,
        currency_id: USDT_CURRENCY_ID,
        amount: "10000.0".to_string(),
    });
    let increase_a_response = client.increase(increase_a_request).await?;
    let increase_a = increase_a_response.into_inner();
    assert_eq!(increase_a.code, 0, "Failed to increase USDT for user A");
    println!("✓ User A: Added 10000.0 USDT");
    if let Some(balance) = increase_a.data {
        println!("  Balance: total={}, available={}, frozen={}", 
                 balance.value, balance.available, balance.frozen);
    }

    // 2. 创建用户 b，增加 BTC (用于卖 BTC)
    println!("\n--- Step 2: Create user B and add BTC ---");
    let increase_b_request = Request::new(IncreaseRequest {
        request_id: 2,
        account_id: ACCOUNT_B,
        currency_id: BTC_CURRENCY_ID,
        amount: "1.0".to_string(),
    });
    let increase_b_response = client.increase(increase_b_request).await?;
    let increase_b = increase_b_response.into_inner();
    assert_eq!(increase_b.code, 0, "Failed to increase BTC for user B");
    println!("✓ User B: Added 1.0 BTC");
    if let Some(balance) = increase_b.data {
        println!("  Balance: total={}, available={}, frozen={}", 
                 balance.value, balance.available, balance.frozen);
    }

    // 等待一下确保余额更新完成
    sleep(Duration::from_millis(100)).await;

    // 3. 用户 a 下买单 (BID)
    println!("\n--- Step 3: User A places buy order ---");
    let buy_order_request = Request::new(PlaceOrderRequest {
        request_id: 3,
        symbol_id: SYMBOL_ID,
        account_id: ACCOUNT_A,
        r#type: Type::Limit as i32,
        side: Side::Bid as i32,
        price: Some("50000.0".to_string()),
        quantity: Some("0.1".to_string()),
        volume: None,
        taker_rate: None,
        maker_rate: None,
    });
    let buy_order_response = client.place_order(buy_order_request).await?;
    let buy_order = buy_order_response.into_inner();
    assert_eq!(buy_order.code, 0, "Failed to place buy order");
    println!("✓ User A: Placed buy order (ID: {})", buy_order.id);
    println!("  Order: BID 0.1 BTC @ 50000 USDT");

    // 等待一下确保订单处理完成
    sleep(Duration::from_millis(200)).await;

    // 4. 用户 b 下卖单 (ASK) - 应该立即成交
    println!("\n--- Step 4: User B places sell order (should match immediately) ---");
    let sell_order_request = Request::new(PlaceOrderRequest {
        request_id: 4,
        symbol_id: SYMBOL_ID,
        account_id: ACCOUNT_B,
        r#type: Type::Limit as i32,
        side: Side::Ask as i32,
        price: Some("50000.0".to_string()),
        quantity: Some("0.1".to_string()),
        volume: None,
        taker_rate: None,
        maker_rate: None,
    });
    let sell_order_response = client.place_order(sell_order_request).await?;
    let sell_order = sell_order_response.into_inner();
    assert_eq!(sell_order.code, 0, "Failed to place sell order");
    println!("✓ User B: Placed sell order (ID: {})", sell_order.id);
    println!("  Order: ASK 0.1 BTC @ 50000 USDT");

    // 等待一下确保交易结算完成
    sleep(Duration::from_millis(500)).await;

    // 5. 检查订单簿 (应该为空，因为订单已成交)
    println!("\n--- Step 5: Check order book ---");
    let orderbook_request = Request::new(GetOrderBookRequest {
        request_id: 5,
        symbol_id: SYMBOL_ID,
        levels: Some(10),
    });
    let orderbook_response = client.get_order_book(orderbook_request).await?;
    let orderbook = orderbook_response.into_inner();
    assert_eq!(orderbook.code, 0, "Failed to get order book");
    println!("✓ Order book retrieved");
    println!("  Bids: {} levels", orderbook.bids.len());
    println!("  Asks: {} levels", orderbook.asks.len());
    if let Some(best_bid) = &orderbook.best_bid {
        println!("  Best bid: {}", best_bid);
    }
    if let Some(best_ask) = &orderbook.best_ask {
        println!("  Best ask: {}", best_ask);
    }
    if let Some(spread) = &orderbook.spread {
        println!("  Spread: {}", spread);
    }

    // 6. 检查用户 a 的余额
    println!("\n--- Step 6: Check User A balance ---");
    let account_a_request = Request::new(GetAccountRequest {
        account_id: ACCOUNT_A,
        currency_id: None,
    });
    let account_a_response = client.get_account(account_a_request).await?;
    let account_a = account_a_response.into_inner();
    assert_eq!(account_a.code, 0, "Failed to get account A");
    println!("✓ User A balance:");
    for (currency_id, balance) in &account_a.data {
        let currency_name = match *currency_id {
            1 => "BTC",
            2 => "USDT",
            _ => "Unknown",
        };
        println!("  {} ({}): total={}, available={}, frozen={}", 
                 currency_name, currency_id, balance.value, balance.available, balance.frozen);
    }

    // 验证用户 a 的余额
    // 用户 a 是买方：应该增加 0.1 BTC，减少 5000 USDT (0.1 * 50000)
    if let Some(btc_balance) = account_a.data.get(&BTC_CURRENCY_ID) {
        let btc_total: f64 = btc_balance.value.parse().unwrap();
        assert!(
            (btc_total - 0.1).abs() < 0.0001,
            "User A BTC should be ~0.1, got {}",
            btc_total
        );
        println!("  ✓ BTC balance correct: ~0.1 BTC");
    }
    if let Some(usdt_balance) = account_a.data.get(&USDT_CURRENCY_ID) {
        let usdt_total: f64 = usdt_balance.value.parse().unwrap();
        // 初始 10000 USDT，下单时冻结了 5000 USDT，成交后扣除
        // 所以 USDT 应该是 5000
        assert!(
            (usdt_total - 5000.0).abs() < 0.01,
            "User A USDT should be ~5000, got {}",
            usdt_total
        );
        println!("  ✓ USDT balance correct: ~5000 USDT");
    }

    // 7. 检查用户 b 的余额
    println!("\n--- Step 7: Check User B balance ---");
    let account_b_request = Request::new(GetAccountRequest {
        account_id: ACCOUNT_B,
        currency_id: None,
    });
    let account_b_response = client.get_account(account_b_request).await?;
    let account_b = account_b_response.into_inner();
    assert_eq!(account_b.code, 0, "Failed to get account B");
    println!("✓ User B balance:");
    for (currency_id, balance) in &account_b.data {
        let currency_name = match *currency_id {
            1 => "BTC",
            2 => "USDT",
            _ => "Unknown",
        };
        println!("  {} ({}): total={}, available={}, frozen={}", 
                 currency_name, currency_id, balance.value, balance.available, balance.frozen);
    }

    // 验证用户 b 的余额
    // 用户 b 是卖方：应该减少 0.1 BTC，增加 5000 USDT
    if let Some(btc_balance) = account_b.data.get(&BTC_CURRENCY_ID) {
        let btc_total: f64 = btc_balance.value.parse().unwrap();
        // 初始 1.0 BTC，成交后减少 0.1，应该是 0.9
        assert!(
            (btc_total - 0.9).abs() < 0.0001,
            "User B BTC should be ~0.9, got {}",
            btc_total
        );
        println!("  ✓ BTC balance correct: ~0.9 BTC");
    }
    if let Some(usdt_balance) = account_b.data.get(&USDT_CURRENCY_ID) {
        let usdt_total: f64 = usdt_balance.value.parse().unwrap();
        // 初始没有 USDT，成交后增加 5000
        assert!(
            (usdt_total - 5000.0).abs() < 0.01,
            "User B USDT should be ~5000, got {}",
            usdt_total
        );
        println!("  ✓ USDT balance correct: ~5000 USDT");
    }

    println!("\n=== All tests passed! ===");
    Ok(())
}



