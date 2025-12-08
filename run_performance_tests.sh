#!/bin/bash

# Lightning 性能测试运行脚本
# 用于执行完整的微秒级性能验证和基准测试

set -e

echo "🚀 Lightning 性能测试套件"
echo "========================================"
echo "开始执行完整的性能验证测试..."
echo ""

# 检查 Rust 环境
if ! command -v cargo &> /dev/null; then
    echo "❌ 错误: 未找到 cargo 命令，请安装 Rust"
    exit 1
fi

echo "✓ Rust 环境检查通过"

# 检查项目目录
if [[ ! -f "Cargo.toml" ]]; then
    echo "❌ 错误: 未在 Lightning 项目根目录中运行"
    exit 1
fi

echo "✓ 项目目录检查通过"

# 编译项目（发布模式以获得最佳性能）
echo ""
echo "📦 编译项目 (Release 模式)..."
cargo build --release

if [[ $? -ne 0 ]]; then
    echo "❌ 编译失败"
    exit 1
fi

echo "✅ 编译完成"

# 创建结果目录
RESULTS_DIR="performance_results_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$RESULTS_DIR"
echo "📁 结果将保存到: $RESULTS_DIR"

# 函数：运行测试并保存结果
run_test() {
    local test_name="$1"
    local test_command="$2"
    local output_file="$RESULTS_DIR/${test_name}_$(date +%H%M%S).log"

    echo ""
    echo "🔬 运行测试: $test_name"
    echo "----------------------------------------"

    # 运行测试并同时输出到终端和文件
    $test_command 2>&1 | tee "$output_file"

    if [[ $? -eq 0 ]]; then
        echo "✅ $test_name 完成"
    else
        echo "❌ $test_name 失败"
    fi
}

# 1. 微秒级延迟验证器
run_test "微秒级延迟验证" "cargo run --release --example microsecond_latency_validator"

# 2. 实时性能监控
run_test "实时性能监控" "timeout 60s cargo run --release --example realtime_performance_monitor || true"

# 3. 完整集成测试（包含性能测试）
run_test "完整功能集成测试" "cargo run --release --example full_integration_test"

# 4. Criterion 基准测试
if cargo bench --list &>/dev/null; then
    echo ""
    echo "🏃 运行 Criterion 基准测试..."
    run_test "Criterion基准测试" "cargo bench"
else
    echo "⚠️  跳过 Criterion 基准测试 (未配置)"
fi

# 5. 单元测试
echo ""
echo "🧪 运行单元测试..."
run_test "单元测试" "cargo test --release"

# 生成性能报告总结
echo ""
echo "📊 生成性能测试总结报告..."

SUMMARY_FILE="$RESULTS_DIR/performance_summary.md"

cat > "$SUMMARY_FILE" << EOF
# Lightning 性能测试报告

**测试时间**: $(date)
**测试环境**: $(uname -a)
**Rust版本**: $(rustc --version)

## 测试概述

本报告包含了 Lightning 高性能交易系统的完整性能验证结果。

### 性能目标

- **撮合延迟**: < 10μs (微秒级)
- **订单提交**: < 10ms (P99延迟)
- **吞吐量**: > 100,000 TPS
- **Level2查询**: < 1ms

### 测试结果文件

EOF

# 列出所有结果文件
for file in "$RESULTS_DIR"/*.log; do
    if [[ -f "$file" ]]; then
        filename=$(basename "$file")
        echo "- [$filename](./$filename)" >> "$SUMMARY_FILE"
    fi
done

cat >> "$SUMMARY_FILE" << EOF

## 快速性能指标提取

以下是关键性能指标的快速提取命令：

\`\`\`bash
# 查看撮合延迟
grep "平均延迟" $RESULTS_DIR/*延迟验证*.log

# 查看P99延迟
grep "P99延迟" $RESULTS_DIR/*延迟验证*.log

# 查看TPS
grep "TPS" $RESULTS_DIR/*性能监控*.log

# 查看性能目标达成情况
grep -E "(✅|❌).*目标" $RESULTS_DIR/*.log
\`\`\`

## 性能优化建议

请查看各个测试日志中的优化建议部分，特别关注：

1. **延迟优化**: 查找 "🔧" 标记的优化建议
2. **吞吐量优化**: 关注并发处理相关建议
3. **内存优化**: 注意内存分配和缓存优化建议

## 基准对比

| 指标 | 目标值 | 当前值 | 状态 |
|------|--------|--------|------|
| 撮合延迟 | < 10μs | - | 请查看日志 |
| 订单提交 | < 10ms | - | 请查看日志 |
| 吞吐量 | > 100k TPS | - | 请查看日志 |
| Level2查询 | < 1ms | - | 请查看日志 |

*具体数值请参考各测试日志文件*

EOF

echo "✅ 性能测试总结报告已生成: $SUMMARY_FILE"

# 显示快速结果概览
echo ""
echo "🎯 快速结果概览"
echo "========================================"

# 尝试从日志中提取关键指标
if find "$RESULTS_DIR" -name "*延迟验证*.log" -exec grep -l "平均延迟" {} \; | head -1 | xargs -I {} grep "平均延迟" {} 2>/dev/null; then
    echo "找到延迟数据 ✓"
else
    echo "延迟数据提取失败 ⚠️"
fi

if find "$RESULTS_DIR" -name "*性能监控*.log" -exec grep -l "TPS" {} \; | head -1 | xargs -I {} grep "当前TPS" {} | tail -5 2>/dev/null; then
    echo "找到TPS数据 ✓"
else
    echo "TPS数据提取失败 ⚠️"
fi

# 统计成功/失败的目标
PASSED_TARGETS=$(find "$RESULTS_DIR" -name "*.log" -exec grep -h "✅.*目标" {} \; | wc -l)
FAILED_TARGETS=$(find "$RESULTS_DIR" -name "*.log" -exec grep -h "❌.*目标" {} \; | wc -l)

echo ""
echo "性能目标达成统计:"
echo "✅ 通过: $PASSED_TARGETS"
echo "❌ 未达到: $FAILED_TARGETS"

if [[ $FAILED_TARGETS -eq 0 && $PASSED_TARGETS -gt 0 ]]; then
    echo "🏆 恭喜! 所有性能目标均已达成"
elif [[ $PASSED_TARGETS -gt $FAILED_TARGETS ]]; then
    echo "👍 大部分性能目标已达成，仍有优化空间"
else
    echo "🔧 需要进行显著的性能优化"
fi

echo ""
echo "📂 完整结果查看:"
echo "   cd $RESULTS_DIR"
echo "   cat performance_summary.md"
echo ""
echo "🚀 Lightning 性能测试完成!"

# 可选：自动打开结果目录
if command -v open &> /dev/null; then
    # macOS
    echo "📁 自动打开结果目录..."
    open "$RESULTS_DIR"
elif command -v xdg-open &> /dev/null; then
    # Linux
    echo "📁 自动打开结果目录..."
    xdg-open "$RESULTS_DIR"
fi

exit 0
