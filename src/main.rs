mod balance;
mod grpc;
mod messages;
mod processor;

use crossbeam_channel;
use grpc::create_server;
use messages::{MatchMessage, SequencerMessage};
use processor::{MatchProcessor, MessageProcessor};
use std::thread;
use tonic::transport::Server;

pub const SHARD_COUNT: usize = 10;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting High-Performance Lightning Balance Service...");

    // 创建高性能channel列表
    let mut message_senders = Vec::new();
    let mut processor_handles = Vec::new();

    // 创建撮合引擎channel列表
    let mut match_senders = Vec::new();
    let mut match_handles = Vec::new();

    // 启动撮合引擎处理器
    for i in 0..SHARD_COUNT {
        let (match_sender, match_receiver) = crossbeam_channel::unbounded::<MatchMessage>();
        match_senders.push(match_sender);

        let processor = MatchProcessor::new(i, match_receiver);
        let handle = thread::spawn(move || {
            processor.run();
        });
        match_handles.push(handle);
    }

    // 启动高性能消息处理器
    for i in 0..SHARD_COUNT {
        let (message_sender, message_receiver) = crossbeam_channel::unbounded::<SequencerMessage>();
        message_senders.push(message_sender);

        let processor = MessageProcessor::new(i, message_receiver, match_senders.clone());
        let handle = thread::spawn(move || {
            processor.run();
        });
        processor_handles.push(handle);
    }

    // 创建高性能gRPC服务
    let grpc_service = create_server(message_senders, SHARD_COUNT);

    // 配置高性能服务器
    let addr = "0.0.0.0:50051".parse()?;
    println!("High-performance gRPC server listening on {}", addr);

    // 使用tokio的并发运行时
    let server = Server::builder().add_service(grpc_service).serve(addr);

    // 启动服务器
    tokio::select! {
        result = server => {
            if let Err(e) = result {
                eprintln!("Server error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("Shutting down server...");
        }
    }

    // 等待处理器线程结束
    for handle in processor_handles {
        handle.join().unwrap();
    }

    // 等待撮合引擎线程结束
    for handle in match_handles {
        handle.join().unwrap();
    }

    Ok(())
}
