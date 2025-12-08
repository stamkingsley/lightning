use lightning::{
    matching::MatchingEngine,
    models::{init_global_config, BalanceManager},
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// 高精度延迟验证器 - 专门验证微秒级性能目标
struct MicrosecondLatencyValidator {
    samples: Vec<Duration>,
    target_avg_micros: f64,
    target_p99_millis: f64,
    target_p999_micros: f64,
}

impl MicrosecondLatencyValidator {
    fn new() -> Self {
        Self {
            samples: Vec::new(),
            target_avg_micros: 10.0,   // 平均撮合延迟 < 10μs
            target_p99_millis: 10.0,   // P99订单提交 < 10ms
            target_p999_micros: 100.0, // P99.9延迟 < 100μs
        }
    }

    fn record_sample(&mut self, latency: Duration) {
        self.samples.push(latency);
    }

    fn analyze_and_validate(&mut self) -> ValidationReport {
        if self.samples.is_empty() {
            return ValidationReport::empty();
        }

        // 排序样本用于百分位计算
        self.samples.sort();

        let count = self.samples.len();
        let total_nanos: u128 = self.samples.iter().map(|d| d.as_nanos()).sum();

        // 基本统计
        let min_latency = self.samples[0];
        let max_latency = self.samples[count - 1];
        let avg_latency = Duration::from_nanos((total_nanos / count as u128) as u64);

        // 百分位延迟
        let p50_latency = self.samples[count * 50 / 100];
        let p95_latency = self.samples[count * 95 / 100];
        let p99_latency = self.samples[count * 99 / 100];
        let p999_latency = self.samples[count * 999 / 1000];

        // 转换为不同单位
        let min_micros = min_latency.as_nanos() as f64 / 1000.0;
        let max_micros = max_latency.as_nanos() as f64 / 1000.0;
        let avg_micros = avg_latency.as_nanos() as f64 / 1000.0;
        let p50_micros = p50_latency.as_nanos() as f64 / 1000.0;
        let p95_micros = p95_latency.as_nanos() as f64 / 1000.0;
        let p99_micros = p99_latency.as_nanos() as f64 / 1000.0;
        let p999_micros = p999_latency.as_nanos() as f64 / 1000.0;

        let p99_millis = p99_micros / 1000.0;

        // 性能目标验证
        let avg_target_met = avg_micros < self.target_avg_micros;
        let p99_target_met = p99_millis < self.target_p99_millis;
        let p999_target_met = p999_micros < self.target_p999_micros;

        // 延迟分布分析
        let under_1us = self.samples.iter().filter(|d| d.as_nanos() < 1000).count();
        let under_10us = self.samples.iter().filter(|d| d.as_nanos() < 10000).count();
        let under_100us = self
            .samples
            .iter()
            .filter(|d| d.as_nanos() < 100000)
            .count();
        let under_1ms = self
            .samples
            .iter()
            .filter(|d| d.as_nanos() < 1000000)
            .count();
        let over_10ms = self
            .samples
            .iter()
            .filter(|d| d.as_nanos() > 10000000)
            .count();

        ValidationReport {
            sample_count: count,
            min_micros,
            max_micros,
            avg_micros,
            p50_micros,
            p95_micros,
            p99_micros,
            p999_micros,
            p99_millis,
            avg_target_met,
            p99_target_met,
            p999_target_met,
            under_1us_count: under_1us,
            under_10us_count: under_10us,
            under_100us_count: under_100us,
            under_1ms_count: under_1ms,
            over_10ms_count: over_10ms,
            targets: ValidationTargets {
                avg_micros: self.target_avg_micros,
                p99_millis: self.target_p99_millis,
                p999_micros: self.target_p999_micros,
            },
        }
    }
}

#[derive(Debug)]
struct ValidationTargets {
    avg_micros: f64,
    p99_millis: f64,
    p999_micros: f64,
}

#[derive(Debug)]
struct ValidationReport {
    sample_count: usize,
    min_micros: f64,
    max_micros: f64,
    avg_micros: f64,
    p50_micros: f64,
    p95_micros: f64,
    p99_micros: f64,
    p999_micros: f64,
    p99_millis: f64,
    avg_target_met: bool,
    p99_target_met: bool,
    p999_target_met: bool,
    under_1us_count: usize,
    under_10us_count: usize,
    under_100us_count: usize,
    under_1ms_count: usize,
    over_10ms_count: usize,
    targets: ValidationTargets,
}

impl ValidationReport {
    fn empty() -> Self {
        Self {
            sample_count: 0,
            min_micros: 0.0,
            max_micros: 0.0,
            avg_micros: 0.0,
            p50_micros: 0.0,
            p95_micros: 0.0,
            p99_micros: 0.0,
            p999_micros: 0.0,
            p99_millis: 0.0,
            avg_target_met: false,
            p99_target_met: false,
            p999_target_met: false,
            under_1us_count: 0,
            under_10us_count: 0,
            under_100us_count: 0,
            under_1ms_count: 0,
            over_10ms_count: 0,
            targets: ValidationTargets {
                avg_micros: 10.0,
                p99_millis: 10.0,
                p999_micros: 100.0,
            },
        }
    }

    fn print_detailed_report(&self, test_name: &str) {
        println!("\n╔═══════════════════════════════════════════════════════════╗");
        println!("║               {} 微秒级延迟验证报告", test_name);
        println!("╠═══════════════════════════════════════════════════════════╣");
        println!("║ 样本统计");
        println!("║   总样本数: {:>10}", self.sample_count);
        println!("║   最小延迟: {:>10.3} μs", self.min_micros);
        println!("║   最大延迟: {:>10.3} μs", self.max_micros);
        println!("║   平均延迟: {:>10.3} μs", self.avg_micros);
        println!("╠═══════════════════════════════════════════════════════════╣");
        println!("║ 百分位延迟");
        println!("║   P50 延迟: {:>10.3} μs", self.p50_micros);
        println!("║   P95 延迟: {:>10.3} μs", self.p95_micros);
        println!(
            "║   P99 延迟: {:>10.3} μs ({:.3} ms)",
            self.p99_micros, self.p99_millis
        );
        println!("║   P99.9延迟: {:>9.3} μs", self.p999_micros);
        println!("╠═══════════════════════════════════════════════════════════╣");
        println!("║ 延迟分布");
        println!(
            "║   < 1 μs   : {:>10} ({:>5.1}%)",
            self.under_1us_count,
            self.under_1us_count as f64 / self.sample_count as f64 * 100.0
        );
        println!(
            "║   < 10 μs  : {:>10} ({:>5.1}%)",
            self.under_10us_count,
            self.under_10us_count as f64 / self.sample_count as f64 * 100.0
        );
        println!(
            "║   < 100 μs : {:>10} ({:>5.1}%)",
            self.under_100us_count,
            self.under_100us_count as f64 / self.sample_count as f64 * 100.0
        );
        println!(
            "║   < 1 ms   : {:>10} ({:>5.1}%)",
            self.under_1ms_count,
            self.under_1ms_count as f64 / self.sample_count as f64 * 100.0
        );
        println!(
            "║   > 10 ms  : {:>10} ({:>5.1}%)",
            self.over_10ms_count,
            self.over_10ms_count as f64 / self.sample_count as f64 * 100.0
        );
        println!("╠═══════════════════════════════════════════════════════════╣");
        println!("║ 性能目标验证");
        println!(
            "║   撮合延迟目标 (<{:.0}μs): {} {:.3}μs",
            self.targets.avg_micros,
            if self.avg_target_met { "✅" } else { "❌" },
            self.avg_micros
        );
        println!(
            "║   订单提交目标 (<{:.0}ms): {} {:.3}ms",
            self.targets.p99_millis,
            if self.p99_target_met { "✅" } else { "❌" },
            self.p99_millis
        );
        println!(
            "║   极端延迟控制 (<{:.0}μs): {} {:.3}μs",
            self.targets.p999_micros,
            if self.p999_target_met { "✅" } else { "❌" },
            self.p999_micros
        );
        println!("╠═══════════════════════════════════════════════════════════╣");
        println!("║ 综合评级");

        let passed_count = [
            self.avg_target_met,
            self.p99_target_met,
            self.p999_target_met,
        ]
        .iter()
        .filter(|&&x| x)
        .count();

        let grade = match passed_count {
            3 => ("A+", "优秀", "🏆"),
            2 => ("B+", "良好", "👍"),
            1 => ("C+", "一般", "⚠️"),
            _ => ("D", "待改进", "⚡"),
        };

        println!("║   性能等级: {} - {} {}", grade.2, grade.1, grade.0);
        println!(
            "║   通过率  : {}/3 ({:.0}%)",
            passed_count,
            passed_count as f64 / 3.0 * 100.0
        );
        println!("╚═══════════════════════════════════════════════════════════╝");
    }

    fn get_optimization_suggestions(&self) -> Vec<String> {
        let mut suggestions = Vec::new();

        if !self.avg_target_met {
            suggestions.push(format!(
                "🔧 撮合延迟优化 (当前: {:.2}μs, 目标: <{:.0}μs):",
                self.avg_micros, self.targets.avg_micros
            ));
            suggestions.push("   - 使用更高效的订单簿数据结构 (如跳表或B+树)".to_string());
            suggestions.push("   - 减少动态内存分配，使用内存池".to_string());
            suggestions.push("   - 启用CPU亲和性绑定".to_string());
            suggestions.push("   - 优化缓存局部性，重排数据结构".to_string());
        }

        if !self.p99_target_met {
            suggestions.push(format!(
                "🔧 订单提交优化 (当前: {:.2}ms, 目标: <{:.0}ms):",
                self.p99_millis, self.targets.p99_millis
            ));
            suggestions.push("   - 优化消息队列性能，减少队列深度".to_string());
            suggestions.push("   - 增加处理器分片数量，提高并行度".to_string());
            suggestions.push("   - 使用无锁数据结构替代互斥锁".to_string());
            suggestions.push("   - 预分配大对象，避免GC压力".to_string());
        }

        if !self.p999_target_met {
            suggestions.push(format!(
                "🔧 极端延迟控制 (当前: {:.2}μs, 目标: <{:.0}μs):",
                self.p999_micros, self.targets.p999_micros
            ));
            suggestions.push("   - 实施延迟预算控制".to_string());
            suggestions.push("   - 增加实时监控和熔断机制".to_string());
            suggestions.push("   - 使用专用线程池避免调度延迟".to_string());
        }

        if self.over_10ms_count > 0 {
            suggestions.push("🚨 高延迟样本分析:".to_string());
            suggestions.push(format!(
                "   - 检测到 {} 个 >10ms 延迟样本 ({:.2}%)",
                self.over_10ms_count,
                self.over_10ms_count as f64 / self.sample_count as f64 * 100.0
            ));
            suggestions.push("   - 建议增加系统监控，识别延迟峰值原因".to_string());
        }

        suggestions
    }
}

/// 核心撮合引擎延迟测试
fn validate_matching_engine_latency() -> ValidationReport {
    println!("🔬 开始核心撮合引擎延迟验证...");

    init_global_config();
    let mut matching_engine = MatchingEngine::new();
    let mut validator = MicrosecondLatencyValidator::new();

    // 预热阶段
    println!("   预热系统...");
    for i in 0..1000 {
        let _ = matching_engine.place_order(
            Uuid::new_v4(),
            1,
            9000 + i,
            0,
            i % 2,
            &format!("{}", 50000 + i),
            "0.001",
        );
    }

    // 实际测试 - 专注于纯撮合算法性能
    println!("   执行高精度延迟测试...");
    for i in 0..50000 {
        let account_id = 10000 + (i % 1000);
        let price = 48000.0 + (i % 2000) as f64;
        let side = i % 2;

        // 高精度时间测量
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

        validator.record_sample(latency);
    }

    validator.analyze_and_validate()
}

/// 完整订单提交流程延迟测试
fn validate_full_order_submission_latency() -> ValidationReport {
    println!("🔬 开始完整订单提交流程延迟验证...");

    init_global_config();
    let mut matching_engine = MatchingEngine::new();
    let mut balance_manager = BalanceManager::new();
    let mut validator = MicrosecondLatencyValidator::new();

    // 预充值账户
    println!("   预充值测试账户...");
    for i in 0..5000 {
        let account_id = 20000 + i;
        balance_manager.handle_increase(account_id, 1, "100.0");
        balance_manager.handle_increase(account_id, 2, "10000000.0");
    }

    // 完整流程测试
    println!("   执行完整订单提交流程测试...");
    for i in 0..20000 {
        let account_id = 20000 + (i % 5000);
        let price = 49000.0 + (i % 1000) as f64;
        let side = i % 2;

        let start = Instant::now();

        // 模拟完整订单提交流程
        // 1. 余额检查
        let balance_result = balance_manager.handle_get_account(account_id, None);

        if balance_result.code == 0 {
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
        validator.record_sample(latency);
    }

    validator.analyze_and_validate()
}

/// 并发场景下的延迟验证
fn validate_concurrent_latency() -> ValidationReport {
    println!("🔬 开始并发场景延迟验证...");

    init_global_config();
    let mut matching_engine = MatchingEngine::new();
    let mut validator = MicrosecondLatencyValidator::new();

    // 并发模拟测试
    println!("   执行高并发延迟测试...");
    for batch in 0..50 {
        // 每批快速提交1000个订单，模拟高并发
        for i in 0..1000 {
            let account_id = 30000 + batch * 1000 + i;
            let price = 50000.0 + (i % 500) as f64;
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

            validator.record_sample(latency);
        }

        if (batch + 1) % 10 == 0 {
            println!("   完成批次: {}/50", batch + 1);
        }
    }

    validator.analyze_and_validate()
}

/// Level2查询延迟验证
fn validate_level2_query_latency() -> ValidationReport {
    println!("🔬 开始Level2查询延迟验证...");

    init_global_config();
    let mut matching_engine = MatchingEngine::new();
    let mut validator = MicrosecondLatencyValidator::new();

    // 构建深度订单簿
    println!("   构建测试订单簿...");
    for i in 0..2000 {
        let account_id = 40000 + i;
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
            account_id + 10000,
            0,
            1,
            &sell_price.to_string(),
            "0.1",
        );
    }

    // Level2查询延迟测试
    println!("   执行Level2查询延迟测试...");
    for _ in 0..20000 {
        let start = Instant::now();

        if let Some(order_book) = matching_engine.get_order_book(1) {
            let _ = order_book.get_market_depth(20);
            let _ = order_book.get_best_bid();
            let _ = order_book.get_best_ask();
            let _ = order_book.get_spread();
        }

        let latency = start.elapsed();
        validator.record_sample(latency);
    }

    validator.analyze_and_validate()
}

fn main() {
    println!("🚀 Lightning 微秒级延迟验证器");
    println!("=====================================");
    println!("专业级性能验证工具");
    println!("目标: 验证微秒级撮合延迟和毫秒级订单提交");
    println!("=====================================\n");

    // 执行各项验证测试
    let matching_report = validate_matching_engine_latency();
    let full_order_report = validate_full_order_submission_latency();
    let concurrent_report = validate_concurrent_latency();
    let level2_report = validate_level2_query_latency();

    // 打印详细报告
    matching_report.print_detailed_report("核心撮合引擎");
    full_order_report.print_detailed_report("完整订单提交");
    concurrent_report.print_detailed_report("高并发场景");
    level2_report.print_detailed_report("Level2查询");

    // 综合分析和建议
    println!("\n╔═══════════════════════════════════════════════════════════╗");
    println!("║                    综合性能分析报告");
    println!("╠═══════════════════════════════════════════════════════════╣");

    let all_reports = vec![
        ("撮合引擎", &matching_report),
        ("订单提交", &full_order_report),
        ("高并发", &concurrent_report),
        ("Level2查询", &level2_report),
    ];

    for (name, report) in &all_reports {
        let status = if report.avg_target_met && report.p99_target_met {
            "✅ 达标"
        } else {
            "❌ 待优化"
        };
        println!(
            "║ {:12}: {} (平均: {:.2}μs, P99: {:.2}ms)",
            name, status, report.avg_micros, report.p99_millis
        );
    }

    println!("╠═══════════════════════════════════════════════════════════╣");
    println!("║                    优化建议汇总");
    println!("╠═══════════════════════════════════════════════════════════╣");

    let mut all_suggestions = Vec::new();
    for (_, report) in &all_reports {
        all_suggestions.extend(report.get_optimization_suggestions());
    }

    if all_suggestions.is_empty() {
        println!("║ 🎉 恭喜！所有性能指标均已达标");
        println!("║    系统已达到微秒级交易处理能力");
    } else {
        for suggestion in all_suggestions {
            println!("║ {}", suggestion);
        }
    }

    println!("╚═══════════════════════════════════════════════════════════╝");

    // 最终评级
    let total_passed = all_reports
        .iter()
        .map(|(_, r)| {
            [r.avg_target_met, r.p99_target_met]
                .iter()
                .filter(|&&x| x)
                .count()
        })
        .sum::<usize>();
    let total_tests = all_reports.len() * 2;

    println!("\n🎯 最终验证结果");
    println!("===============================");
    println!("通过测试: {}/{}", total_passed, total_tests);
    println!(
        "通过率: {:.1}%",
        total_passed as f64 / total_tests as f64 * 100.0
    );

    if total_passed == total_tests {
        println!("🏆 系统性能评级: A+ (微秒级交易系统)");
        println!("✨ 已达到高频交易系统标准！");
    } else if total_passed >= total_tests * 3 / 4 {
        println!("🥈 系统性能评级: B+ (高性能交易系统)");
        println!("💪 接近微秒级目标，需要进一步优化");
    } else {
        println!("⚡ 系统性能评级: C+ (标准交易系统)");
        println!("🔧 需要显著性能优化才能达到微秒级目标");
    }

    println!("\n📊 验证完成！");
    println!("详细性能数据可用于生产环境调优参考。");
}
