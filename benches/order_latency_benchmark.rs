use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use lightning::{
    matching::MatchingEngine,
    models::{init_global_config, BalanceManager},
};
use std::time::{Duration, Instant};
use uuid::Uuid;

// 高精度延迟测量器
struct LatencyMeasurer {
    samples: Vec<Duration>,
    min: Duration,
    max: Duration,
    sum: Duration,
}

impl LatencyMeasurer {
    fn new() -> Self {
        Self {
            samples: Vec::new(),
            min: Duration::from_secs(1),
            max: Duration::from_nanos(0),
            sum: Duration::from_nanos(0),
        }
    }

    fn record(&mut self, latency: Duration) {
        self.samples.push(latency);
        self.min = self.min.min(latency);
        self.max = self.max.max(latency);
        self.sum += latency;
    }

    fn percentile(&mut self, p: f64) -> Duration {
        self.samples.sort();
        let index = ((self.samples.len() as f64) * p / 100.0) as usize;
        self.samples[index.min(self.samples.len() - 1)]
    }

    fn average(&self) -> Duration {
        if self.samples.is_empty() {
            Duration::from_nanos(0)
        } else {
            self.sum / self.samples.len() as u32
        }
    }

    fn report(&mut self, name: &str) {
        println!("\n=== {} 延迟报告 ===", name);
        println!("样本数量: {}", self.samples.len());
        println!(
            "最小延迟: {:?} ({:.2}μs)",
            self.min,
            self.min.as_nanos() as f64 / 1000.0
        );
        println!(
            "最大延迟: {:?} ({:.2}μs)",
            self.max,
            self.max.as_nanos() as f64 / 1000.0
        );
        println!(
            "平均延迟: {:?} ({:.2}μs)",
            self.average(),
            self.average().as_nanos() as f64 / 1000.0
        );
        println!(
            "P50延迟: {:?} ({:.2}μs)",
            self.percentile(50.0),
            self.percentile(50.0).as_nanos() as f64 / 1000.0
        );
        println!(
            "P95延迟: {:?} ({:.2}μs)",
            self.percentile(95.0),
            self.percentile(95.0).as_nanos() as f64 / 1000.0
        );
        println!(
            "P99延迟: {:?} ({:.2}μs)",
            self.percentile(99.0),
            self.percentile(99.0).as_nanos() as f64 / 1000.0
        );
        println!(
            "P99.9延迟: {:?} ({:.2}μs)",
            self.percentile(99.9),
            self.percentile(99.9).as_nanos() as f64 / 1000.0
        );

        // 验证性能目标
        let p99_micros = self.percentile(99.0).as_nanos() as f64 / 1000.0;
        let avg_micros = self.average().as_nanos() as f64 / 1000.0;

        println!("\n=== 性能目标验证 ===");
        if avg_micros < 10.0 {
            println!("✅ 撮合延迟目标 (<10μs): 通过 ({:.2}μs)", avg_micros);
        } else {
            println!("❌ 撮合延迟目标 (<10μs): 未达到 ({:.2}μs)", avg_micros);
        }

        let p99_millis = p99_micros / 1000.0;
        if p99_millis < 10.0 {
            println!("✅ 订单提交目标 (<10ms): 通过 ({:.2}ms)", p99_millis);
        } else {
            println!("❌ 订单提交目标 (<10ms): 未达到 ({:.2}ms)", p99_millis);
        }
    }
}

// 测试订单撮合延迟（核心撮合算法）
fn benchmark_matching_latency() {
    init_global_config();
    let mut matching_engine = MatchingEngine::new();
    let mut latency_measurer = LatencyMeasurer::new();

    println!("=== 微秒级撮合延迟测试 ===");

    // 预热系统
    for _ in 0..1000 {
        let _ = matching_engine.place_order(Uuid::new_v4(), 1, 1001, 0, 0, "50000.0", "0.001");
    }

    // 实际测试：测量纯撮合算法延迟
    for i in 0..10000 {
        let account_id = 2000 + (i % 100);
        let price = 48000.0 + (i % 1000) as f64;
        let side = i % 2;

        let start = Instant::now();
        let _ = matching_engine.place_order(
            Uuid::new_v4(),
            1,
            account_id,
            0, // Limit order
            side,
            &price.to_string(),
            "0.001",
        );
        let latency = start.elapsed();

        latency_measurer.record(latency);
    }

    latency_measurer.report("订单撮合");
}

// 测试完整订单提交流程延迟（包含余额检查）
fn benchmark_full_order_latency() {
    init_global_config();
    let mut matching_engine = MatchingEngine::new();
    let mut balance_manager = BalanceManager::new();
    let mut latency_measurer = LatencyMeasurer::new();

    println!("\n=== 完整订单提交延迟测试 ===");

    // 预充值账户
    for i in 0..1000 {
        let account_id = 3000 + i;
        balance_manager.handle_increase(account_id, 1, "100.0"); // BTC
        balance_manager.handle_increase(account_id, 2, "5000000.0"); // USDT
    }

    // 测试完整订单流程
    for i in 0..5000 {
        let account_id = 3000 + (i % 1000);
        let price = 48000.0 + (i % 2000) as f64;
        let side = i % 2;

        let start = Instant::now();

        // 模拟完整订单提交流程
        // 1. 余额检查
        let balance_check = balance_manager.handle_get_account(account_id, None);
        if balance_check.code == 0 {
            // 2. 订单撮合
            let _ = matching_engine.place_order(
                Uuid::new_v4(),
                1,
                account_id,
                0,
                side,
                &price.to_string(),
                "0.001",
            );
        }

        let latency = start.elapsed();
        latency_measurer.record(latency);
    }

    latency_measurer.report("完整订单提交");
}

// 测试高并发场景下的延迟
fn benchmark_concurrent_latency() {
    init_global_config();
    let mut matching_engine = MatchingEngine::new();
    let mut latency_measurer = LatencyMeasurer::new();

    println!("\n=== 高并发延迟测试 ===");

    // 模拟高并发：快速连续提交订单
    for batch in 0..10 {
        let batch_start = Instant::now();

        for i in 0..1000 {
            let account_id = 4000 + i;
            let price = 49000.0 + (i % 500) as f64;
            let side = i % 2;

            let start = Instant::now();
            let _ = matching_engine.place_order(
                Uuid::new_v4(),
                1,
                account_id,
                0,
                side,
                &price.to_string(),
                "0.001",
            );
            let latency = start.elapsed();

            latency_measurer.record(latency);
        }

        let batch_duration = batch_start.elapsed();
        let tps = 1000.0 / batch_duration.as_secs_f64();
        println!(
            "批次 {}: 1000订单用时 {:?}, TPS: {:.0}",
            batch + 1,
            batch_duration,
            tps
        );
    }

    latency_measurer.report("高并发场景");
}

// 测试Level2查询延迟
fn benchmark_level2_latency() {
    init_global_config();
    let mut matching_engine = MatchingEngine::new();
    let mut latency_measurer = LatencyMeasurer::new();

    println!("\n=== Level2查询延迟测试 ===");

    // 填充订单簿
    for i in 0..1000 {
        let account_id = 5000 + i;
        let buy_price = 48000.0 + i as f64;
        let sell_price = 52000.0 + i as f64;

        let _ = matching_engine.place_order(
            Uuid::new_v4(),
            1,
            account_id,
            0,
            0,
            &buy_price.to_string(),
            "0.1",
        );
        let _ = matching_engine.place_order(
            Uuid::new_v4(),
            1,
            account_id + 1000,
            0,
            1,
            &sell_price.to_string(),
            "0.1",
        );
    }

    // 测试查询延迟
    for _ in 0..10000 {
        let start = Instant::now();

        if let Some(order_book) = matching_engine.get_order_book(1) {
            let _ = order_book.get_market_depth(20);
            let _ = order_book.get_best_bid();
            let _ = order_book.get_best_ask();
            let _ = order_book.get_spread();
        }

        let latency = start.elapsed();
        latency_measurer.record(latency);
    }

    latency_measurer.report("Level2查询");
}

// 压力测试：极限吞吐量下的延迟表现
fn benchmark_stress_latency() {
    init_global_config();
    let mut matching_engine = MatchingEngine::new();
    let mut latency_measurer = LatencyMeasurer::new();

    println!("\n=== 极限压力延迟测试 ===");

    let total_orders = 50000;
    let start_time = Instant::now();

    for i in 0..total_orders {
        let account_id = 6000 + (i % 10000);
        let price = 50000.0 + (i as f64 % 1000.0);
        let side = i % 2;

        let order_start = Instant::now();
        let _ = matching_engine.place_order(
            Uuid::new_v4(),
            1,
            account_id,
            0,
            side,
            &price.to_string(),
            "0.001",
        );
        let order_latency = order_start.elapsed();

        latency_measurer.record(order_latency);

        // 每1000个订单报告一次进度
        if (i + 1) % 10000 == 0 {
            let elapsed = start_time.elapsed();
            let current_tps = (i + 1) as f64 / elapsed.as_secs_f64();
            println!("已处理 {} 订单, 当前TPS: {:.0}", i + 1, current_tps);
        }
    }

    let total_duration = start_time.elapsed();
    let final_tps = total_orders as f64 / total_duration.as_secs_f64();

    println!("压力测试完成:");
    println!("总订单数: {}", total_orders);
    println!("总耗时: {:?}", total_duration);
    println!("最终TPS: {:.0}", final_tps);

    latency_measurer.report("极限压力测试");
}

// Criterion基准测试
fn criterion_benchmark(c: &mut Criterion) {
    init_global_config();

    let mut group = c.benchmark_group("order_processing");
    group.throughput(Throughput::Elements(1));

    // 基准测试：单个订单处理
    group.bench_function("single_order", |b| {
        let mut matching_engine = MatchingEngine::new();
        let mut counter = 0u64;

        b.iter(|| {
            counter += 1;
            let account_id = 1000 + (counter % 1000);
            let price = 50000.0 + (counter % 100) as f64;

            black_box(matching_engine.place_order(
                Uuid::new_v4(),
                1,
                account_id,
                0,
                0,
                &price.to_string(),
                "0.001",
            ))
        });
    });

    // 基准测试：Level2查询
    group.bench_function("level2_query", |b| {
        let mut matching_engine = MatchingEngine::new();

        // 预填充订单簿
        for i in 0..100 {
            let _ = matching_engine.place_order(
                Uuid::new_v4(),
                1,
                7000 + i,
                0,
                i % 2,
                &format!("{}", 50000 + i * 10),
                "0.1",
            );
        }

        b.iter(|| {
            if let Some(order_book) = matching_engine.get_order_book(1) {
                black_box((
                    order_book.get_market_depth(5),
                    order_book.get_best_bid(),
                    order_book.get_best_ask(),
                ));
            }
        });
    });

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

// 集成测试主函数
pub fn run_all_benchmarks() {
    println!("🚀 Lightning 微秒级性能基准测试");
    println!("========================================");

    benchmark_matching_latency();
    benchmark_full_order_latency();
    benchmark_concurrent_latency();
    benchmark_level2_latency();
    benchmark_stress_latency();

    println!("\n=== 性能基准测试总结 ===");
    println!("✅ 所有基准测试完成");
    println!("📊 详细报告已生成");
    println!("🎯 性能目标验证完成");

    println!("\n=== 建议的性能优化 ===");
    println!("1. 如果平均延迟 > 10μs，考虑优化数据结构");
    println!("2. 如果P99延迟 > 10ms，需要优化内存分配");
    println!("3. 如果TPS < 100,000，考虑增加分片数量");
    println!("4. 使用CPU亲和性和内存预分配进一步优化");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_measurer() {
        let mut measurer = LatencyMeasurer::new();

        measurer.record(Duration::from_micros(5));
        measurer.record(Duration::from_micros(10));
        measurer.record(Duration::from_micros(15));

        assert_eq!(measurer.min, Duration::from_micros(5));
        assert_eq!(measurer.max, Duration::from_micros(15));
        assert_eq!(measurer.average(), Duration::from_micros(10));
    }

    #[test]
    fn test_benchmark_execution() {
        // 快速验证基准测试能正常运行
        init_global_config();
        let mut matching_engine = MatchingEngine::new();

        let start = Instant::now();
        let result = matching_engine.place_order(Uuid::new_v4(), 1, 8888, 0, 0, "50000.0", "0.001");
        let duration = start.elapsed();

        assert!(result.is_ok());
        assert!(duration < Duration::from_millis(1)); // 基本延迟检查
    }
}
