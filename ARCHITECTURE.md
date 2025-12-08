# Lightning Balance Service - 系统架构

## 🏗️ 正确的无锁并发架构

### 核心设计原理

**并发安全的关键**: 所有余额操作必须通过单一的 SequencerProcessor 进行，避免多个处理器同时操作同一账户余额。

### 架构图

```
                    gRPC 请求
                        │
                        ▼
            ┌─────────────────────────┐
            │      gRPC Server        │
            │   (负载均衡+路由)         │
            └─────────────────────────┘
                        │
                        ▼ 按账户ID分片路由
    ┌───────────────────────────────────────────────────────┐
    │                SequencerProcessor 分片                  │
    │              (按账户ID % 10 分片)                       │
    └───────────────────────────────────────────────────────┘
            │                                    │
            │ 1. 订单前处理                       │ 4. 成交后处理
            │   - 余额验证                        │   - 余额更新
            │   - 资金冻结                        │   - 资金转移
            ▼                                    ▲
┌─────────────────┐                    ┌─────────────────┐
│SequencerProcessor│◄──────────────────┤TradeExecutionMsg│
│       0-9        │    成交回调消息      │   (消息队列)     │
└─────────────────┘                    └─────────────────┘
            │                                    ▲
            │ 2. 转发到撮合引擎                    │ 3. 成交结果
            ▼                                    │
    ┌───────────────────────────────────────────────────────┐
    │              MatchProcessor 分片                       │
    │             (按交易对ID % 10 分片)                      │
    └───────────────────────────────────────────────────────┘
                        │
                        ▼
            ┌─────────────────────────┐
            │    MatchingEngine       │
            │    (订单簿+撮合算法)      │
            └─────────────────────────┘
```

### 数据流详解

#### 1. 订单提交流程
```
用户下单 → gRPC Server → SequencerProcessor[账户ID%10]
                                      │
                                      ▼
                              余额验证+资金冻结
                                      │
                                      ▼ (成功)
                        MatchProcessor[交易对ID%10]
                                      │
                                      ▼
                               MatchingEngine 撮合
```

#### 2. 成交执行流程
```
MatchingEngine 产生成交 → MatchProcessor
                               │
                               ▼
                      TradeExecutionMsg 消息
                               │
                               ▼
             SequencerProcessor[买方账户ID%10] (买方余额更新)
                               │
                               ▼
             SequencerProcessor[卖方账户ID%10] (卖方余额更新)
```

## 🔒 并发安全保证

### 分片策略

#### SequencerProcessor 分片 (余额管理)
- **分片键**: `account_id % SHARD_COUNT`
- **职责**: 管理特定范围账户的余额
- **优势**: 确保同一账户只由一个处理器管理，避免竞争

#### MatchProcessor 分片 (订单撮合)
- **分片键**: `symbol_id % SHARD_COUNT`
- **职责**: 管理特定交易对的订单簿和撮合
- **优势**: 同一交易对的订单串行处理，保证撮合顺序

### 消息传递机制

```rust
// 1. 用户订单消息
SequencerMessage::PlaceOrder {
    account_id,     // 用于路由到对应的 SequencerProcessor
    symbol_id,      // 转发给对应的 MatchProcessor
    // ... 其他字段
}

// 2. 撮合转发消息
MatchMessage::PlaceOrder {
    symbol_id,      // 用于 MatchProcessor 分片
    // ... 订单信息
}

// 3. 成交执行消息
TradeExecutionMessage::ExecuteTrade {
    trade: Trade {
        buy_account_id,   // 路由到买方 SequencerProcessor
        sell_account_id,  // 路由到卖方 SequencerProcessor
    }
}
```

## 🚀 性能优化

### 无锁设计
- **原理**: 每个账户只由一个 SequencerProcessor 管理
- **结果**: 完全避免了传统锁机制的开销
- **性能**: 实现真正的并行处理

### 内存布局优化
```rust
// SequencerProcessor 内部结构
struct SequencerProcessor {
    id: usize,                                    // 分片ID
    balance_manager: BalanceManager,              // 独立的余额管理器
    receiver: Receiver<SequencerMessage>,        // 主消息队列
    trade_receiver: Receiver<TradeExecutionMsg>, // 成交执行消息队列
}

// 双消息队列处理
loop {
    select! {
        recv(self.receiver) -> msg => {
            // 处理订单消息
            self.process_sequencer_message(msg);
        }
        recv(self.trade_receiver) -> trade_msg => {
            // 处理成交执行
            self.process_trade_execution(trade_msg);
        }
    }
}
```

## 📊 容量规划

### 处理器配置
- **SequencerProcessor**: 10个实例
  - 每个管理约 10% 的账户
  - 独立的余额管理器实例
- **MatchProcessor**: 10个实例
  - 每个管理约 10% 的交易对
  - 独立的撮合引擎实例

### 扩展能力
- **账户扩展**: 增加 SequencerProcessor 实例数量
- **交易对扩展**: 增加 MatchProcessor 实例数量
- **动态调整**: 支持运行时调整分片数量

## 🛡️ 故障处理

### 消息可靠性
- 使用 `crossbeam-channel` 确保消息传递可靠性
- 内存队列避免了外部依赖
- 通道关闭时优雅退出

### 数据一致性
- 成交执行通过消息回调确保原子性
- 同一处理器内操作天然具备 ACID 特性
- 分片间通过消息队列松耦合

## 🔧 监控指标

### 性能指标
- 每个处理器的消息队列长度
- 订单处理延迟
- 撮合执行时间
- 余额更新耗时

### 业务指标
- 订单成功率
- 成交匹配率
- 账户余额准确性
- 系统吞吐量

## 📈 未来扩展

### 持久化支持
- 可在每个 SequencerProcessor 中添加本地持久化
- 支持 WAL (Write-Ahead Logging)
- 故障恢复机制

### 集群部署
- 支持多节点部署
- 跨节点的账户分片
- 分布式撮合引擎

这种架构确保了在高并发场景下的数据安全性，同时保持了极高的性能表现。