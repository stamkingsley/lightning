use lightning::{
    matching::{MatchingEngine, OrderSide, OrderType},
    models::{init_global_config, BalanceManager},
};
use rust_decimal::Decimal;
use std::collections::HashMap;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    println!("=== Lightning Matching Engine Demo ===\n");

    // 初始化全局配置
    init_global_config();
    println!("✓ Global currencies and symbols initialized");
    println!("  - BTC (id: 1)");
    println!("  - USDT (id: 2)");
    println!("  - BTC-USDT (symbol_id: 1, base: BTC, quote: USDT)\n");

    // 创建撮合引擎
    let mut matching_engine = MatchingEngine::new();
    println!("✓ Matching engine created\n");

    // 测试账户
    let alice_id = 1001; // Alice 有很多 USDT，想买 BTC
    let bob_id = 1002; // Bob 有 BTC，想卖 BTC
    let charlie_id = 1003; // Charlie 也想买 BTC
    let david_id = 1004; // David 也想卖 BTC

    println!("=== 演示1: 基础限价订单撮合 ===");

    // Alice 下买单: 50,000 USDT 买 1.0 BTC
    println!("1. Alice 下买单: 以 50,000 USDT 价格买 1.0 BTC");
    let (alice_order_id, alice_trades) = matching_engine
        .place_order(
            Uuid::new_v4(),
            1, // BTC-USDT
            alice_id,
            0, // Limit order
            0, // Bid
            "50000.0",
            "1.0",
        )
        .unwrap();

    println!(
        "   订单ID: {}, 成交数: {}",
        alice_order_id,
        alice_trades.len()
    );

    // Bob 下卖单: 51,000 USDT 卖 0.5 BTC (价格太高，不会立即撮合)
    println!("2. Bob 下卖单: 以 51,000 USDT 价格卖 0.5 BTC");
    let (bob_order_id, bob_trades) = matching_engine
        .place_order(
            Uuid::new_v4(),
            1,
            bob_id,
            0, // Limit order
            1, // Ask
            "51000.0",
            "0.5",
        )
        .unwrap();

    println!("   订单ID: {}, 成交数: {}", bob_order_id, bob_trades.len());

    // 显示当前订单簿状态
    print_order_book(&matching_engine, 1);

    // David 下更低价格的卖单，会与 Alice 的买单撮合
    println!("3. David 下卖单: 以 49,000 USDT 价格卖 0.8 BTC (会与Alice撮合)");
    let (david_order_id, david_trades) = matching_engine
        .place_order(
            Uuid::new_v4(),
            1,
            david_id,
            0, // Limit order
            1, // Ask
            "49000.0",
            "0.8",
        )
        .unwrap();

    println!(
        "   订单ID: {}, 成交数: {}",
        david_order_id,
        david_trades.len()
    );

    if !david_trades.is_empty() {
        for trade in &david_trades {
            println!(
                "   ✓ 成交: 买方账户={}, 卖方账户={}, 价格={}, 数量={}",
                trade.buy_account_id, trade.sell_account_id, trade.price, trade.quantity
            );
        }
    }

    print_order_book(&matching_engine, 1);

    println!("\n=== 演示2: 市价订单 ===");

    // Charlie 下市价买单，会吃掉订单簿中最优的卖单
    println!("4. Charlie 下市价买单: 市价买 0.3 BTC");
    let (charlie_order_id, charlie_trades) = matching_engine
        .place_order(
            Uuid::new_v4(),
            1,
            charlie_id,
            1,   // Market order
            0,   // Bid
            "0", // 市价单价格不重要
            "0.3",
        )
        .unwrap();

    println!(
        "   订单ID: {}, 成交数: {}",
        charlie_order_id,
        charlie_trades.len()
    );

    if !charlie_trades.is_empty() {
        for trade in &charlie_trades {
            println!(
                "   ✓ 成交: 买方账户={}, 卖方账户={}, 价格={}, 数量={}",
                trade.buy_account_id, trade.sell_account_id, trade.price, trade.quantity
            );
        }
    }

    print_order_book(&matching_engine, 1);

    println!("\n=== 演示3: 部分成交场景 ===");

    // 添加一些更多的订单来演示部分成交
    println!("5. 添加多个小额卖单");

    let orders_to_add = vec![
        (2001, "52000.0", "0.1"),
        (2002, "52500.0", "0.15"),
        (2003, "53000.0", "0.2"),
    ];

    for (account_id, price, quantity) in orders_to_add {
        let (order_id, trades) = matching_engine
            .place_order(
                Uuid::new_v4(),
                1,
                account_id,
                0, // Limit
                1, // Ask
                price,
                quantity,
            )
            .unwrap();
        println!(
            "   账户 {} 下卖单: 价格={}, 数量={}, 订单ID={}",
            account_id, price, quantity, order_id
        );
    }

    print_order_book(&matching_engine, 1);

    // 现在下一个大的买单，会部分撮合多个卖单
    println!("6. 下大额买单，部分撮合多个卖单");
    let (big_buy_id, big_buy_trades) = matching_engine
        .place_order(
            Uuid::new_v4(),
            1,
            3001,
            0,         // Limit order
            0,         // Bid
            "52800.0", // 高价格，会撮合多个卖单
            "0.4",     // 较大数量
        )
        .unwrap();

    println!(
        "   大额买单ID: {}, 成交数: {}",
        big_buy_id,
        big_buy_trades.len()
    );

    if !big_buy_trades.is_empty() {
        let mut total_volume = Decimal::ZERO;
        let mut total_cost = Decimal::ZERO;

        for trade in &big_buy_trades {
            println!(
                "   ✓ 成交: 买方={}, 卖方={}, 价格={}, 数量={}",
                trade.buy_account_id, trade.sell_account_id, trade.price, trade.quantity
            );
            total_volume += trade.quantity;
            total_cost += trade.price * trade.quantity;
        }

        let avg_price = if total_volume > Decimal::ZERO {
            total_cost / total_volume
        } else {
            Decimal::ZERO
        };

        println!(
            "   📊 总成交量: {}, 平均成交价: {:.2}",
            total_volume, avg_price
        );
    }

    print_order_book(&matching_engine, 1);

    println!("\n=== 演示4: 订单簿深度分析 ===");

    let order_book = matching_engine.get_order_book(1).unwrap();
    let (bids, asks) = order_book.get_market_depth(10);

    println!("买单深度 (Bids):");
    if bids.is_empty() {
        println!("   无买单");
    } else {
        for (i, (price, quantity)) in bids.iter().enumerate() {
            println!("   Level {}: 价格={}, 数量={}", i + 1, price, quantity);
        }
    }

    println!("\n卖单深度 (Asks):");
    if asks.is_empty() {
        println!("   无卖单");
    } else {
        for (i, (price, quantity)) in asks.iter().enumerate() {
            println!("   Level {}: 价格={}, 数量={}", i + 1, price, quantity);
        }
    }

    if let Some(spread) = order_book.get_spread() {
        println!("\n💰 当前买卖价差: {}", spread);
    } else {
        println!("\n💰 当前无买卖价差 (单边市场)");
    }

    println!("\n=== 演示5: 成交历史 ===");

    let recent_trades = matching_engine.get_recent_trades(1, 10);
    println!("最近成交记录 (最多10条):");

    if recent_trades.is_empty() {
        println!("   暂无成交记录");
    } else {
        for (i, trade) in recent_trades.iter().enumerate() {
            println!(
                "   {}. ID={}, 买方={}, 卖方={}, 价格={}, 数量={}, 时间={}",
                i + 1,
                trade.id,
                trade.buy_account_id,
                trade.sell_account_id,
                trade.price,
                trade.quantity,
                trade.created_at
            );
        }
    }

    println!("\n=== 演示6: 错误处理 ===");

    // 测试无效交易对
    println!("7. 测试无效交易对 (symbol_id: 999)");
    match matching_engine.place_order(
        Uuid::new_v4(),
        999, // 不存在的交易对
        4001,
        0,
        0,
        "50000.0",
        "1.0",
    ) {
        Ok(_) => println!("   意外成功"),
        Err(e) => println!("   ✓ 正确拒绝: {}", e),
    }

    // 测试无效价格格式
    println!("8. 测试无效价格格式");
    match matching_engine.place_order(
        Uuid::new_v4(),
        1,
        4002,
        0,
        0,
        "invalid_price", // 无效价格
        "1.0",
    ) {
        Ok(_) => println!("   意外成功"),
        Err(e) => println!("   ✓ 正确拒绝: {}", e),
    }

    println!("\n=== 撮合引擎统计 ===");
    println!("总订单簿数量: {}", matching_engine.order_books.len());
    println!("总成交记录数: {}", matching_engine.trades.len());
    println!("下一个订单ID: {}", matching_engine.next_order_id);

    println!("\n=== 撮合引擎核心特性总结 ===");
    println!("✓ 价格-时间优先级撮合");
    println!("✓ 限价单和市价单支持");
    println!("✓ 部分成交处理");
    println!("✓ 实时订单簿维护");
    println!("✓ 成交历史记录");
    println!("✓ 市场深度分析");
    println!("✓ 错误处理和验证");

    println!("\n=== 演示完成 ===");
}

fn print_order_book(engine: &MatchingEngine, symbol_id: i32) {
    println!("\n📖 当前订单簿状态 (Symbol: {}):", symbol_id);

    if let Some(order_book) = engine.get_order_book(symbol_id) {
        let (bids, asks) = order_book.get_market_depth(5);

        println!("   买单 (Bids):");
        if bids.is_empty() {
            println!("     无");
        } else {
            for (price, quantity) in bids {
                println!("     价格: {}, 数量: {}", price, quantity);
            }
        }

        println!("   卖单 (Asks):");
        if asks.is_empty() {
            println!("     无");
        } else {
            for (price, quantity) in asks {
                println!("     价格: {}, 数量: {}", price, quantity);
            }
        }

        if let (Some(best_bid), Some(best_ask)) =
            (order_book.get_best_bid(), order_book.get_best_ask())
        {
            println!("   最优买价: {}, 最优卖价: {}", best_bid, best_ask);
            println!("   价差: {}", best_ask - best_bid);
        } else if let Some(best_bid) = order_book.get_best_bid() {
            println!("   最优买价: {}, 无卖单", best_bid);
        } else if let Some(best_ask) = order_book.get_best_ask() {
            println!("   最优卖价: {}, 无买单", best_ask);
        } else {
            println!("   订单簿为空");
        }
    } else {
        println!("   订单簿不存在");
    }
    println!();
}
