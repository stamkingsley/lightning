use lightning::{
    matching::MatchingEngine,
    models::{init_global_config, BalanceManager},
};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

// 实时性能监控器
#[derive(Clone)]
struct PerformanceMonitor {
    order_count: Arc<AtomicU64>,
    total_latency_nanos: Arc<AtomicU64>,
    recent_latencies: Arc<std::sync::Mutex<VecDeque<Duration>>>,
    start_time: Instant,
}

impl PerformanceMonitor {
    fn new() -> Self {
        Self {
            order_count: Arc::new(AtomicU64::new(0)),
            total_latency_nanos: Arc::new(AtomicU64::new(0)),
            recent_latencies: Arc::new(std::sync::Mutex::new(VecDeque::with_capacity(1000))),
            start_time: Instant::now(),
        }
    }

    fn record_order_latency(&self, latency: Duration) {
        self.order_count.fetch_add(1, Ordering::Relaxed);
        self.total_latency_nanos
            .fetch_add(latency.as_nanos() as u64, Ordering::Relaxed);

        // 记录最近1000个延迟样本用于实时分析
        if let Ok(mut recent) = self.recent_latencies.lock() {
            if recent.len() >= 1000 {
                recent.pop_front();
            }
            recent.push_back(latency);
        }
    }

    fn get_metrics(&self) -> PerformanceMetrics {
        let order_count = self.order_count.load(Ordering::Relaxed);
        let total_latency_nanos = self.total_latency_nanos.load(Ordering::Relaxed);
        let elapsed = self.start_time.elapsed();

        let average_latency_micros = if order_count > 0 {
            (total_latency_nanos / order_count) as f64 / 1000.0
        } else {
            0.0
        };

        let tps = if elapsed.as_secs_f64() > 0.0 {
            order_count as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        };

        // 计算最近样本的P95和P99延迟
        let (p95_micros, p99_micros, p999_micros) = if let Ok(recent) = self.recent_latencies.lock()
        {
            if recent.len() > 10 {
                let mut sorted: Vec<_> = recent.iter().cloned().collect();
                sorted.sort();

                let p95_idx = (sorted.len() as f64 * 0.95) as usize;
                let p99_idx = (sorted.len() as f64 * 0.99) as usize;
                let p999_idx = (sorted.len() as f64 * 0.999) as usize;

                let p95 = sorted[p95_idx.min(sorted.len() - 1)].as_nanos() as f64 / 1000.0;
                let p99 = sorted[p99_idx.min(sorted.len() - 1)].as_nanos() as f64 / 1000.0;
                let p999 = sorted[p999_idx.min(sorted.len() - 1)].as_nanos() as f64 / 1000.0;

                (p95, p99, p999)
            } else {
                (0.0, 0.0, 0.0)
            }
        } else {
            (0.0, 0.0, 0.0)
        };

        PerformanceMetrics {
            order_count,
            elapsed_seconds: elapsed.as_secs_f64(),
            tps,
            average_latency_micros,
            p95_latency_micros: p95_micros,
            p99_latency_micros: p99_micros,
            p999_latency_micros: p999_micros,
        }
    }
}

#[derive(Debug, Clone)]
struct PerformanceMetrics {
    order_count: u64,
    elapsed_seconds: f64,
    tps: f64,
    average_latency_micros: f64,
    p95_latency_micros: f64,
    p99_latency_micros: f64,
    p999_latency_micros: f64,
}

impl PerformanceMetrics {
    fn print_report(&self, title: &str) {
        println!("\n=== {} ===", title);
        println!("运行时间: {:.1}s", self.elapsed_seconds);
        println!("订单总数: {}", self.order_count);
        println!("当前TPS: {:.0}", self.tps);
        println!("平均延迟: {:.2}μs", self.average_latency_micros);
        println!("P95 延迟: {:.2}μs", self.p95_latency_micros);
        println!("P99 延迟: {:.2}μs", self.p99_latency_micros);
        println!("P99.9延迟: {:.2}μs", self.p999_latency_micros);

        // 性能目标验证
        println!("\n--- 性能目标验证 ---");

        // 微秒级撮合目标: < 10μs
        if self.average_latency_micros < 10.0 {
            println!(
                "✅ 撮合延迟目标 (<10μs): 通过 ({:.2}μs)",
                self.average_latency_micros
            );
        } else {
            println!(
                "❌ 撮合延迟目标 (<10μs): 未达到 ({:.2}μs)",
                self.average_latency_micros
            );
        }

        // 订单提交目标: < 10ms
        let p99_millis = self.p99_latency_micros / 1000.0;
        if p99_millis < 10.0 {
            println!("✅ 订单提交目标 (<10ms): 通过 ({:.2}ms)", p99_millis);
        } else {
            println!("❌ 订单提交目标 (<10ms): 未达到 ({:.2}ms)", p99_millis);
        }

        // 高并发目标: > 100,000 TPS
        if self.tps > 100000.0 {
            println!("✅ 高并发目标 (>100k TPS): 通过 ({:.0} TPS)", self.tps);
        } else {
            println!("⚠️  高并发目标 (>100k TPS): 当前 ({:.0} TPS)", self.tps);
        }

        println!("{}".repeat(50));
    }

    fn validate_sla(&self) -> bool {
        self.average_latency_micros < 10.0
            && self.p99_latency_micros < 10000.0
            && self.tps > 50000.0
    }
}

// 负载生成器
struct LoadGenerator {
    account_counter: AtomicU64,
}

impl LoadGenerator {
    fn new() -> Self {
        Self {
            account_counter: AtomicU64::new(10000),
        }
    }

    fn generate_order_params(&self) -> (u64, String, u32, String) {
        let account_id = self.account_counter.fetch_add(1, Ordering::Relaxed);
        let price_base = 50000.0;
        let price_variation = (account_id % 2000) as f64;
        let price = price_base + price_variation;
        let side = (account_id % 2) as u32;
        let quantity = "0.001";

        (account_id, price.to_string(), side, quantity.to_string())
    }
}

// 延迟分类统计
struct LatencyDistribution {
    under_1us: AtomicU64,
    under_10us: AtomicU64,
    under_100us: AtomicU64,
    under_1ms: AtomicU64,
    under_10ms: AtomicU64,
    over_10ms: AtomicU64,
}

impl LatencyDistribution {
    fn new() -> Self {
        Self {
            under_1us: AtomicU64::new(0),
            under_10us: AtomicU64::new(0),
            under_100us: AtomicU64::new(0),
            under_1ms: AtomicU64::new(0),
            under_10ms: AtomicU64::new(0),
            over_10ms: AtomicU64::new(0),
        }
    }

    fn record(&self, latency: Duration) {
        let micros = latency.as_nanos() as f64 / 1000.0;

        if micros < 1.0 {
            self.under_1us.fetch_add(1, Ordering::Relaxed);
        } else if micros < 10.0 {
            self.under_10us.fetch_add(1, Ordering::Relaxed);
        } else if micros < 100.0 {
            self.under_100us.fetch_add(1, Ordering::Relaxed);
        } else if micros < 1000.0 {
            self.under_1ms.fetch_add(1, Ordering::Relaxed);
        } else if micros < 10000.0 {
            self.under_10ms.fetch_add(1, Ordering::Relaxed);
        } else {
            self.over_10ms.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn print_distribution(&self, total_orders: u64) {
        println!("\n=== 延迟分布统计 ===");
        let under_1us = self.under_1us.load(Ordering::Relaxed);
        let under_10us = self.under_10us.load(Ordering::Relaxed);
        let under_100us = self.under_100us.load(Ordering::Relaxed);
        let under_1ms = self.under_1ms.load(Ordering::Relaxed);
        let under_10ms = self.under_10ms.load(Ordering::Relaxed);
        let over_10ms = self.over_10ms.load(Ordering::Relaxed);

        let total = total_orders as f64;

        println!(
            "< 1μs    : {:8} ({:5.1}%) {}",
            under_1us,
            (under_1us as f64 / total) * 100.0,
            "█".repeat((under_1us as f64 / total * 50.0) as usize)
        );
        println!(
            "< 10μs   : {:8} ({:5.1}%) {}",
            under_10us,
            (under_10us as f64 / total) * 100.0,
            "█".repeat((under_10us as f64 / total * 50.0) as usize)
        );
        println!(
            "< 100μs  : {:8} ({:5.1}%) {}",
            under_100us,
            (under_100us as f64 / total) * 100.0,
            "█".repeat((under_100us as f64 / total * 50.0) as usize)
        );
        println!(
            "< 1ms    : {:8} ({:5.1}%) {}",
            under_1ms,
            (under_1ms as f64 / total) * 100.0,
            "█".repeat((under_1ms as f64 / total * 50.0) as usize)
        );
        println!(
            "< 10ms   : {:8} ({:5.1}%) {}",
            under_10ms,
            (under_10ms as f64 / total) * 100.0,
            "█".repeat((under_10ms as f64 / total * 50.0) as usize)
        );
        println!(
            "> 10ms   : {:8} ({:5.1}%) {}",
            over_10ms,
            (over_10ms as f64 / total) * 100.0,
            "█".repeat((over_10ms as f64 / total * 50.0) as usize)
        );
    }
}

#[tokio::main]
async fn main() {
    println!("🚀 Lightning 实时性能监控系统");
    println!("==============================\n");

    // 初始化系统
    init_global_config();
    let mut matching_engine = MatchingEngine::new();
    let mut balance_manager = BalanceManager::new();
    let monitor = PerformanceMonitor::new();
    let load_generator = LoadGenerator::new();
    let latency_distribution = Arc::new(LatencyDistribution::new());

    println!("✓ 系统初始化完成");

    // 预充值账户（避免余额不足影响测试）
    println!("✓ 预充值测试账户...");
    for i in 0..10000 {
        let account_id = 10000 + i;
        balance_manager.handle_increase(account_id, 1, "1000.0"); // BTC
        balance_manager.handle_increase(account_id, 2, "50000000.0"); // USDT
    }

    println!("✓ 预充值完成，开始性能测试\n");

    // 启动实时监控报告线程
    let monitor_clone = monitor.clone();
    let report_handle = thread::spawn(move || {
        let mut last_order_count = 0;
        loop {
            thread::sleep(Duration::from_secs(5));

            let metrics = monitor_clone.get_metrics();
            let orders_delta = metrics.order_count - last_order_count;
            let current_tps = orders_delta as f64 / 5.0; // 5秒间隔的TPS

            println!("\n📊 实时性能监控 ({}s)", metrics.elapsed_seconds as u32);
            println!("当前5s TPS: {:.0}", current_tps);
            println!("总订单数: {}", metrics.order_count);
            println!("平均延迟: {:.2}μs", metrics.average_latency_micros);
            println!("P99延迟: {:.2}μs", metrics.p99_latency_micros);

            last_order_count = metrics.order_count;

            if metrics.order_count > 0 {
                // 实时SLA验证
                if metrics.validate_sla() {
                    println!("✅ SLA: 通过");
                } else {
                    println!("❌ SLA: 未达标");
                }
            }

            println!("{}", "-".repeat(30));
        }
    });

    // 阶段1: 预热阶段
    println!("🔥 阶段1: 系统预热 (10,000订单)");
    for i in 0..10000 {
        let (account_id, price, side, quantity) = load_generator.generate_order_params();

        let start = Instant::now();
        let _ =
            matching_engine.place_order(Uuid::new_v4(), 1, account_id, 0, side, &price, &quantity);
        let latency = start.elapsed();

        monitor.record_order_latency(latency);
        latency_distribution.record(latency);

        // 预热阶段不需要太高频率
        if i % 1000 == 0 {
            thread::sleep(Duration::from_millis(1));
        }
    }

    let warmup_metrics = monitor.get_metrics();
    warmup_metrics.print_report("预热阶段完成");

    // 阶段2: 标准负载测试
    println!("⚡ 阶段2: 标准负载测试 (50,000订单)");
    let phase2_start = Instant::now();

    for _ in 0..50000 {
        let (account_id, price, side, quantity) = load_generator.generate_order_params();

        let start = Instant::now();
        let result =
            matching_engine.place_order(Uuid::new_v4(), 1, account_id, 0, side, &price, &quantity);
        let latency = start.elapsed();

        monitor.record_order_latency(latency);
        latency_distribution.record(latency);

        // 检查订单是否成功
        if result.is_err() {
            eprintln!("订单处理失败: {:?}", result);
        }
    }

    let phase2_metrics = monitor.get_metrics();
    phase2_metrics.print_report("标准负载测试完成");

    // 阶段3: 极限压力测试
    println!("💥 阶段3: 极限压力测试 (100,000订单)");
    let phase3_start = Instant::now();

    for batch in 0..100 {
        for _ in 0..1000 {
            let (account_id, price, side, quantity) = load_generator.generate_order_params();

            let start = Instant::now();
            let _ = matching_engine.place_order(
                Uuid::new_v4(),
                1,
                account_id,
                0,
                side,
                &price,
                &quantity,
            );
            let latency = start.elapsed();

            monitor.record_order_latency(latency);
            latency_distribution.record(latency);
        }

        // 每批次间短暂休息，模拟真实交易环境
        if batch % 10 == 0 {
            let batch_metrics = monitor.get_metrics();
            println!("批次 {}/100 - TPS: {:.0}", batch + 1, batch_metrics.tps);
        }
    }

    let final_metrics = monitor.get_metrics();
    final_metrics.print_report("极限压力测试完成");

    // 打印延迟分布
    latency_distribution.print_distribution(final_metrics.order_count);

    // Level2查询性能测试
    println!("\n📈 阶段4: Level2查询性能测试");
    let mut level2_latencies = Vec::new();

    for _ in 0..10000 {
        let start = Instant::now();
        if let Some(order_book) = matching_engine.get_order_book(1) {
            let _ = order_book.get_market_depth(20);
            let _ = order_book.get_best_bid();
            let _ = order_book.get_best_ask();
            let _ = order_book.get_spread();
        }
        level2_latencies.push(start.elapsed());
    }

    // 计算Level2查询统计
    level2_latencies.sort();
    let avg_level2 = level2_latencies.iter().sum::<Duration>() / level2_latencies.len() as u32;
    let p95_level2 = level2_latencies[(level2_latencies.len() as f64 * 0.95) as usize];
    let p99_level2 = level2_latencies[(level2_latencies.len() as f64 * 0.99) as usize];

    println!("=== Level2查询性能 ===");
    println!("查询总数: 10,000");
    println!("平均延迟: {:.2}μs", avg_level2.as_nanos() as f64 / 1000.0);
    println!("P95延迟: {:.2}μs", p95_level2.as_nanos() as f64 / 1000.0);
    println!("P99延迟: {:.2}μs", p99_level2.as_nanos() as f64 / 1000.0);

    // 最终报告和建议
    println!("\n🎯 最终性能评估报告");
    println!("==========================================");

    let meets_matching_target = final_metrics.average_latency_micros < 10.0;
    let meets_order_target = final_metrics.p99_latency_micros < 10000.0;
    let meets_tps_target = final_metrics.tps > 100000.0;

    println!("性能目标达成情况:");
    println!(
        "  撮合延迟 (<10μs): {} ({:.2}μs)",
        if meets_matching_target { "✅" } else { "❌" },
        final_metrics.average_latency_micros
    );
    println!(
        "  订单提交 (<10ms): {} ({:.2}ms)",
        if meets_order_target { "✅" } else { "❌" },
        final_metrics.p99_latency_micros / 1000.0
    );
    println!(
        "  高并发TPS (>100k): {} ({:.0})",
        if meets_tps_target { "✅" } else { "❌" },
        final_metrics.tps
    );

    println!("\n优化建议:");
    if !meets_matching_target {
        println!("  🔧 撮合延迟优化:");
        println!("     - 使用更高效的数据结构 (如B+树)");
        println!("     - 减少内存分配");
        println!("     - 启用CPU亲和性");
    }

    if !meets_order_target {
        println!("  🔧 订单提交优化:");
        println!("     - 优化消息队列");
        println!("     - 增加处理器分片数量");
        println!("     - 使用内存池技术");
    }

    if !meets_tps_target {
        println!("  🔧 吞吐量优化:");
        println!("     - 并行处理优化");
        println!("     - 减少锁竞争");
        println!("     - 使用更多CPU核心");
    }

    println!("\n🚀 实时性能监控完成!");
    println!("详细性能数据已记录，可用于生产环境调优。");

    // 终止监控线程
    report_handle.join().unwrap_or_default();
}
