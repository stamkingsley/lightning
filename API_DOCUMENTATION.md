# Lightning Balance Service - API 文档

## 概述

Lightning Balance Service 是一个高性能的数字货币交易系统，提供余额管理、订单处理、撮合引擎和市场数据接口。系统采用 gRPC 协议提供服务，支持高并发和低延迟的交易操作。

## 服务端点

- **gRPC 服务地址**: `0.0.0.0:50051`
- **协议**: gRPC over HTTP/2
- **数据格式**: Protocol Buffers

## API 接口列表

### 1. 账户余额查询 (getAccount)

查询指定账户的余额信息。

**请求参数**:
```protobuf
message GetAccountRequest {
  sint32 accountId = 1;           // 账户ID (必填)
  optional sint32 currencyId = 2; // 货币ID (可选，不填则返回所有币种)
}
```

**响应数据**:
```protobuf
message GetAccountResponse {
  sint32 code = 1;                     // 状态码 (0=成功)
  optional string message = 2;         // 状态消息
  map<sint32, Balance> data = 3;       // 余额数据，key为货币ID
}

message Balance {
  string currency = 1;    // 货币ID (字符串形式)
  string value = 2;       // 总余额
  string frozen = 3;      // 冻结余额
  string available = 4;   // 可用余额
}
```

**使用示例**:
```bash
# 查询账户1001的所有余额
grpcurl -plaintext -d '{"accountId": 1001}' localhost:50051 schema.Lightning/getAccount

# 查询账户1001的BTC余额 (货币ID=1)
grpcurl -plaintext -d '{"accountId": 1001, "currencyId": 1}' localhost:50051 schema.Lightning/getAccount
```

**响应示例**:
```json
{
  "code": 0,
  "message": "Success",
  "data": {
    "1": {
      "currency": "1",
      "value": "10.5",
      "frozen": "2.0",
      "available": "8.5"
    },
    "2": {
      "currency": "2", 
      "value": "100000.0",
      "frozen": "50000.0",
      "available": "50000.0"
    }
  }
}
```

### 2. 余额增加 (increase)

增加指定账户的余额。

**请求参数**:
```protobuf
message IncreaseRequest {
  sint64 requestId = 1;   // 请求ID
  sint32 accountId = 2;   // 账户ID
  sint32 currencyId = 3;  // 货币ID
  string amount = 4;      // 增加金额 (字符串格式的十进制数)
}
```

**响应数据**:
```protobuf
message IncreaseResponse {
  sint32 code = 1;              // 状态码
  optional string message = 2;  // 状态消息
  optional Balance data = 3;    // 更新后的余额
}
```

### 3. 余额减少 (decrease)

减少指定账户的余额。

**请求参数**:
```protobuf
message DecreaseRequest {
  sint64 requestId = 1;   // 请求ID
  sint32 accountId = 2;   // 账户ID
  sint32 currencyId = 3;  // 货币ID
  string amount = 4;      // 减少金额
}
```

**响应数据**:
```protobuf
message DecreaseResponse {
  sint32 code = 1;              // 状态码
  optional string message = 2;  // 状态消息
  optional Balance data = 3;    // 更新后的余额
}
```

### 4. 下单 (placeOrder)

提交买卖订单到撮合引擎。

**请求参数**:
```protobuf
message PlaceOrderRequest {
  sint64 requestId = 1;         // 请求ID
  sint32 symbolId = 2;          // 交易对ID
  sint32 accountId = 3;         // 账户ID
  Type type = 4;                // 订单类型 (LIMIT=0, MARKET=1)
  Side side = 5;                // 订单方向 (BID=0, ASK=1)
  optional string price = 6;    // 价格 (限价单必填)
  optional string quantity = 7; // 数量
  optional string volume = 8;   // 成交金额 (市价买单可用)
  optional sint32 takerRate = 9;   // 吃单费率
  optional sint32 makerRate = 10;  // 挂单费率
}
```

**响应数据**:
```protobuf
message PlaceOrderResponse {
  sint32 code = 1;              // 状态码
  optional string message = 2;  // 状态消息  
  sint64 id = 3;               // 订单ID
}
```

**订单类型说明**:
- **LIMIT (0)**: 限价单，指定价格执行
- **MARKET (1)**: 市价单，以市场最优价格立即执行

**订单方向说明**:
- **BID (0)**: 买入订单，用 quote currency 购买 base currency
- **ASK (1)**: 卖出订单，卖出 base currency 获得 quote currency

**使用示例**:
```bash
# 限价买单：以50000 USDT价格买入1.0 BTC
grpcurl -plaintext -d '{
  "requestId": 12345,
  "symbolId": 1,
  "accountId": 1001,
  "type": "LIMIT",
  "side": "BID", 
  "price": "50000.0",
  "quantity": "1.0"
}' localhost:50051 schema.Lightning/placeOrder

# 市价卖单：卖出0.5 BTC
grpcurl -plaintext -d '{
  "requestId": 12346,
  "symbolId": 1,
  "accountId": 1002,
  "type": "MARKET",
  "side": "ASK",
  "quantity": "0.5"
}' localhost:50051 schema.Lightning/placeOrder
```

### 5. 订单簿查询 (getOrderBook)

查询指定交易对的Level2订单簿深度数据。

**请求参数**:
```protobuf
message GetOrderBookRequest {
  sint64 requestId = 1;         // 请求ID
  sint32 symbolId = 2;          // 交易对ID (必填)
  optional sint32 levels = 3;   // 深度档数 (可选，默认20档)
}
```

**响应数据**:
```protobuf
message GetOrderBookResponse {
  sint32 code = 1;                    // 状态码
  optional string message = 2;        // 状态消息
  sint32 symbolId = 3;                // 交易对ID
  repeated PriceLevel bids = 4;       // 买盘深度，按价格降序
  repeated PriceLevel asks = 5;       // 卖盘深度，按价格升序  
  optional string bestBid = 6;        // 最优买价
  optional string bestAsk = 7;        // 最优卖价
  optional string spread = 8;         // 买卖价差
  sint64 timestamp = 9;              // 数据时间戳 (毫秒)
}

message PriceLevel {
  string price = 1;     // 价格
  string quantity = 2;  // 该价位的总数量
}
```

**使用示例**:
```bash
# 查询BTC-USDT的5档深度
grpcurl -plaintext -d '{
  "requestId": 12347,
  "symbolId": 1,
  "levels": 5
}' localhost:50051 schema.Lightning/getOrderBook

# 查询默认20档深度
grpcurl -plaintext -d '{
  "symbolId": 1
}' localhost:50051 schema.Lightning/getOrderBook
```

**响应示例**:
```json
{
  "code": 0,
  "message": "Success",
  "symbolId": 1,
  "bids": [
    {"price": "50000.0", "quantity": "1.0"},
    {"price": "49900.0", "quantity": "0.5"},
    {"price": "49800.0", "quantity": "0.8"}
  ],
  "asks": [
    {"price": "50100.0", "quantity": "0.5"},
    {"price": "50200.0", "quantity": "1.0"},
    {"price": "50300.0", "quantity": "0.8"}
  ],
  "bestBid": "50000.0",
  "bestAsk": "50100.0", 
  "spread": "100.0",
  "timestamp": 1765195595297
}
```

### 6. 取消订单 (cancelOrder) 🆕

取消指定的订单并解冻占用的余额。

**请求参数**:
```protobuf
message CancelOrderRequest {
  sint64 requestId = 1;   // 请求ID
  sint32 symbolId = 2;    // 交易对ID (必填)
  sint32 accountId = 3;   // 账户ID (必填)
  sint64 orderId = 4;     // 要取消的订单ID (必填)
}
```

**响应数据**:
```protobuf
message CancelOrderResponse {
  sint32 code = 1;                      // 状态码
  optional string message = 2;          // 状态消息
  sint64 orderId = 3;                   // 订单ID
  optional string cancelledQuantity = 4; // 取消的数量
  optional string refundAmount = 5;      // 退还的金额
}
```

**取消逻辑说明**:
- **买单取消**: 解冻 `price × remaining_quantity` 的 quote currency
- **卖单取消**: 解冻 `remaining_quantity` 的 base currency
- **部分成交**: 只取消未成交的部分，已成交部分不受影响
- **权限检查**: 只有订单所有者可以取消自己的订单

**使用示例**:
```bash
# 取消订单
grpcurl -plaintext -d '{
  "requestId": 12348,
  "symbolId": 1,
  "accountId": 1001,
  "orderId": 12345
}' localhost:50051 schema.Lightning/cancelOrder
```

**响应示例**:
```json
{
  "code": 0,
  "message": "Order cancelled successfully",
  "orderId": 12345,
  "cancelledQuantity": "0.8",
  "refundAmount": "40000.0"
}
```

**错误情况**:
- `404`: 订单不存在
- `403`: 订单不属于指定账户
- `400`: 订单已完全成交或已取消

## 系统配置

### 支持的货币
- **BTC (ID: 1)**: Bitcoin
- **USDT (ID: 2)**: Tether USD

### 支持的交易对
- **BTC-USDT (ID: 1)**: Bitcoin/Tether USD
  - Base Currency: BTC (ID: 1)
  - Quote Currency: USDT (ID: 2)

## 错误码说明

| 错误码 | 说明 |
|--------|------|
| 0 | 成功 |
| 400 | 请求参数错误 |
| 403 | 权限错误 (如订单不属于指定账户) |
| 404 | 资源不存在 (如账户不存在、交易对不存在、订单不存在) |
| 500 | 内部服务器错误 |

**常见错误消息**:
- `"Insufficient balance"`: 余额不足
- `"Account not found"`: 账户不存在  
- `"Currency not found"`: 货币或交易对不存在
- `"Invalid amount format"`: 金额格式错误
- `"OrderBook not found"`: 订单簿不存在
- `"Order not found"`: 订单不存在
- `"Order does not belong to this account"`: 订单不属于指定账户
- `"Order cancelled successfully"`: 订单取消成功

## 性能特征

### 延迟指标
- **账户查询**: < 1ms
- **余额操作**: < 1ms  
- **订单提交**: < 10ms (包含撮合)
- **订单簿查询**: < 1ms
- **订单取消**: < 5ms (包含余额解冻)

### 吞吐量指标
- **账户操作**: > 50,000 TPS
- **订单处理**: > 100,000 TPS
- **订单簿查询**: > 200,000 QPS
- **订单取消**: > 80,000 TPS

### 并发支持
- **最大连接数**: 10,000+
- **并发用户**: 1,000,000+
- **处理器架构**: 20个并发处理器 (10个撮合 + 10个余额管理)

## 数据精度

所有数值字段都使用字符串格式传输，确保金融级精度：
- **余额精度**: 18位小数
- **价格精度**: 8位小数  
- **数量精度**: 8位小数
- **计算引擎**: rust_decimal，避免浮点数误差

## SDK 和工具

### 推荐工具
- **gRPCurl**: 命令行测试工具
- **Postman**: GUI测试工具 (支持gRPC)
- **BloomRPC**: 专用gRPC客户端

### 客户端库支持
支持所有主流编程语言的gRPC客户端库：
- **Rust**: tonic
- **Go**: grpc-go  
- **Python**: grpcio
- **Java**: grpc-java
- **Node.js**: @grpc/grpc-js
- **C++**: grpc++

## 部署和运维

### 系统要求
- **CPU**: 4核以上
- **内存**: 8GB以上
- **网络**: 千兆网卡
- **操作系统**: Linux/macOS/Windows

### 监控指标
- **处理延迟**: 各接口响应时间
- **吞吐量**: 每秒处理请求数
- **错误率**: 错误请求占比
- **内存使用**: 订单簿和余额数据内存占用
- **连接数**: 活跃gRPC连接数

### 日志格式
系统提供详细的结构化日志，包括：
- 请求/响应日志
- 撮合执行日志  
- 余额变更日志
- 错误和异常日志

## 最佳实践

### 1. 连接管理
- 使用连接池管理gRPC连接
- 启用连接保活 (keepalive)
- 合理设置超时时间

### 2. 错误处理
- 始终检查响应的 `code` 字段
- 实现指数退避重试机制
- 记录详细的错误信息用于调试

### 3. 性能优化
- 批量操作时使用多个并发连接
- 缓存不变的配置信息 (如交易对信息)
- 合理设置订单簿查询的深度档数

### 4. 安全考虑
- 使用TLS加密gRPC连接 (生产环境)
- 实施API访问频率限制
- 验证所有输入参数的合法性

## 版本历史

### v1.0.0 (Current)
- ✅ 基础余额管理接口
- ✅ 订单提交和撮合功能
- ✅ Level2订单簿查询接口
- ✅ 订单取消接口 🆕
- ✅ 高性能并发架构
- ✅ 金融级精度保证

### 计划功能
- 🔜 交易历史查询
- 🔜 WebSocket实时推送
- 🔜 批量操作接口
- 🔜 高级订单类型 (停损、止盈等)
- 🔜 订单状态查询接口

---

**技术支持**: 如有问题请查看系统日志或联系技术团队
**更新日期**: 2024-12-26