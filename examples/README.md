# Integration Test

This integration test demonstrates a complete trading flow:

1. Create user A and add USDT (for buying BTC)
2. Create user B and add BTC (for selling BTC)
3. User A places a buy order (BID)
4. User B places a sell order (ASK) - should match immediately
5. Check order book (should be empty after matching)
6. Check balances for both users

## How to Run

1. Start the Lightning server:
   ```bash
   cargo run
   ```

2. In another terminal, run the integration test:
   ```bash
   cargo run --example integration_test
   ```

## Expected Results

- User A (buyer):
  - BTC: ~0.1 (gained from trade)
  - USDT: ~5000 (10000 - 5000 frozen for order)

- User B (seller):
  - BTC: ~0.9 (1.0 - 0.1 sold)
  - USDT: ~5000 (gained from trade)

- Order book: Empty (orders matched immediately)



