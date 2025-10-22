use schema::lightning_client::LightningClient;
use schema::{DecreaseRequest, GetAccountRequest, IncreaseRequest};
use tonic::Request;

mod schema {
    tonic::include_proto!("schema");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = LightningClient::connect("http://127.0.0.1:50051").await?;

    println!("Testing Lightning Balance Service...");

    // 测试查询不存在的账户
    println!("\n1. Testing get_account for non-existent account...");
    let request = Request::new(GetAccountRequest {
        account_id: 999,
        currency_id: Some(1),
    });
    let response = client.get_account(request).await?;
    println!(
        "Get non-existent account response: {:?}",
        response.get_ref()
    );

    // 测试增加余额
    println!("\n2. Testing increase...");
    let request = Request::new(IncreaseRequest {
        request_id: 1,
        account_id: 1,
        currency_id: 1,
        amount: "100.50".to_string(),
    });
    let response = client.increase(request).await?;
    println!("Increase response: {:?}", response.get_ref());

    // 增加另一种币种
    println!("\n3. Testing increase different currency...");
    let request = Request::new(IncreaseRequest {
        request_id: 2,
        account_id: 1,
        currency_id: 2,
        amount: "200.00".to_string(),
    });
    let response = client.increase(request).await?;
    println!("Increase currency 2 response: {:?}", response.get_ref());

    // 查询特定币种
    println!("\n4. Testing get_account specific currency...");
    let request = Request::new(GetAccountRequest {
        account_id: 1,
        currency_id: Some(1),
    });
    let response = client.get_account(request).await?;
    println!("Get account currency 1 response: {:?}", response.get_ref());

    // 查询所有币种
    println!("\n5. Testing get_account all currencies...");
    let request = Request::new(GetAccountRequest {
        account_id: 1,
        currency_id: None,
    });
    let response = client.get_account(request).await?;
    println!(
        "Get account all currencies response: {:?}",
        response.get_ref()
    );

    // 测试减少余额
    println!("\n6. Testing decrease...");
    let request = Request::new(DecreaseRequest {
        request_id: 3,
        account_id: 1,
        currency_id: 1,
        amount: "50.25".to_string(),
    });
    let response = client.decrease(request).await?;
    println!("Decrease response: {:?}", response.get_ref());

    // 最终查询所有币种
    println!("\n7. Final get_account all currencies...");
    let request = Request::new(GetAccountRequest {
        account_id: 1,
        currency_id: None,
    });
    let response = client.get_account(request).await?;
    println!(
        "Final get account all currencies response: {:?}",
        response.get_ref()
    );

    // 测试余额不足的情况
    println!("\n8. Testing insufficient balance...");
    let request = Request::new(DecreaseRequest {
        request_id: 4,
        account_id: 1,
        currency_id: 1,
        amount: "1000.00".to_string(),
    });
    let response = client.decrease(request).await?;
    println!("Insufficient balance response: {:?}", response.get_ref());

    Ok(())
}
