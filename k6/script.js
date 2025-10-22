import grpc from 'k6/net/grpc';
import { check, sleep } from 'k6';
import { SharedArray } from 'k6/data';

// 初始化 gRPC 客户端
const client = new grpc.Client();
client.load(['../schema/proto'], 'lightning.proto');
console.log('Proto files loaded successfully');

// 加载测试数据
const testData = new SharedArray('testData', function () {
  const data = [];
  for (let i = 0; i < 10000; i++) {
    data.push({
      accountId: 100 + i, // 100 到 10199
      currencyId: (i % 3) + 1, // 1, 2, 3 循环
      requestId: 1000 + i, // 1000 到 10999
    });
  }
  return data;
});

// 配置 k6 执行器：20 个 VU，10,000 次请求
export const options = {
  executor: 'shared-iterations',
  vus: 20, // 20 个虚拟用户
  iterations: 100000, // 总共 10,000 次请求
  maxDuration: '30m', // 最大运行时间
  thresholds: {
    'grpc_req_duration': ['p(95)<200'], // 95% 请求小于 200ms
    'checks': ['rate>0.95'], // 95% 检查通过
  },
};

export function setup() {
  // 在 setup 中不建立连接，交给每个 VU 管理
  console.log('Setup completed, connection will be managed per VU');
}

export default () => {
  // 每个 VU 在首次迭代时建立连接
  if (__ITER === 0) {
    try {
      client.connect('127.0.0.1:50051', { plaintext: true, timeout: '10s' });
      console.log(`VU ${__VU} connected at iteration ${__ITER}`);
    } catch (error) {
      console.error(`VU ${__VU} failed to connect at iteration ${__ITER}:`, error);
      sleep(1);
      return; // 跳过当前迭代
    }
  }

  // 获取动态测试数据
  const data = testData[__ITER % testData.length];

  // 1. 调用 getAccount
  const getAccountRequest = {
    accountId: data.accountId,
    currencyId: data.currencyId,
  };
  let getAccountResponse;
  try {
    getAccountResponse = client.invoke('schema.Lightning/getAccount', getAccountRequest);
    check(getAccountResponse, {
      'getAccount status is OK': (r) => r && r.status === grpc.StatusOK,
      'getAccount has data': (r) => r.message && r.message.data !== undefined,
    });
    console.log(`VU ${__VU} getAccount response (accountId=${data.accountId}):`, JSON.stringify(getAccountResponse.message));
  } catch (error) {
    console.error(`VU ${__VU} getAccount invoke failed (accountId=${data.accountId}):`, error);
  }

  // 2. 调用 increase
  const increaseRequest = {
    requestId: data.requestId,
    accountId: data.accountId,
    currencyId: data.currencyId,
    amount: '100.50',
  };
  let increaseResponse;
  try {
    increaseResponse = client.invoke('schema.Lightning/increase', increaseRequest);
    check(increaseResponse, {
      'increase status is OK': (r) => r && r.status === grpc.StatusOK,
      'increase has data': (r) => r.message && r.message.data !== undefined,
    });
    console.log(`VU ${__VU} increase response (accountId=${data.accountId}):`, JSON.stringify(increaseResponse.message));
  } catch (error) {
    console.error(`VU ${__VU} increase invoke failed (accountId=${data.accountId}):`, error);
  }

  // 3. 调用 decrease
  const decreaseRequest = {
    requestId: data.requestId + 1,
    accountId: data.accountId,
    currencyId: data.currencyId,
    amount: '50.25',
  };
  let decreaseResponse;
  try {
    decreaseResponse = client.invoke('schema.Lightning/decrease', decreaseRequest);
    check(decreaseResponse, {
      'decrease status is OK': (r) => r && r.status === grpc.StatusOK,
      'decrease has data': (r) => r.message && r.message.data !== undefined,
    });
    console.log(`VU ${__VU} decrease response (accountId=${data.accountId}):`, JSON.stringify(decreaseResponse.message));
  } catch (error) {
    console.error(`VU ${__VU} decrease invoke failed (accountId=${data.accountId}):`, error);
  }

//   sleep(0.2); // 暂停 200ms，控制请求速率
};

export function teardown() {
  console.log('Closing gRPC connection');
  client.close();
}