mod grpc;
mod matching;
mod messages;
mod models;
mod processor;

use crossbeam_channel;
use grpc::create_server;
use messages::{MatchMessage, SequencerMessage, TradeExecutionMessage};
use models::init_global_config;
use processor::{MatchProcessor, SequencerProcessor};
use std::thread;
use tonic::transport::Server;

pub const SHARD_COUNT: usize = 10;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting High-Performance Lightning Balance Service...");

    // 初始化全局配置
    init_global_config();
    println!("Global currencies and symbols initialized");

    // 创建高性能channel列表
    let mut sequencer_senders = Vec::new();
    let mut processor_handles = Vec::new();

    // 创建撮合引擎channel列表
    let mut match_senders = Vec::new();
    let mut match_handles = Vec::new();

    // 创建成交执行channel列表 - 每个SequencerProcessor一个
    let mut trade_execution_senders = Vec::new();
    let mut trade_execution_receivers = Vec::new();

    for _ in 0..SHARD_COUNT {
        let (sender, receiver) = crossbeam_channel::unbounded::<TradeExecutionMessage>();
        trade_execution_senders.push(sender);
        trade_execution_receivers.push(receiver);
    }

    // 启动高性能消息处理器（SequencerProcessor）
    for i in 0..SHARD_COUNT {
        let (message_sender, message_receiver) = crossbeam_channel::unbounded::<SequencerMessage>();
        sequencer_senders.push(message_sender);

        let processor = SequencerProcessor::new(
            i,
            message_receiver,
            match_senders.clone(),
            trade_execution_receivers.remove(0),
        );
        let handle = thread::spawn(move || {
            processor.run();
        });
        processor_handles.push(handle);
    }

    // 启动撮合引擎处理器
    for i in 0..SHARD_COUNT {
        let (match_sender, match_receiver) = crossbeam_channel::unbounded::<MatchMessage>();
        match_senders.push(match_sender);

        let processor = MatchProcessor::new(i, match_receiver, trade_execution_senders.clone());
        let handle = thread::spawn(move || {
            processor.run();
        });
        match_handles.push(handle);
    }

    // 创建高性能gRPC服务
    let grpc_service = create_server(sequencer_senders.clone(), match_senders.clone(), SHARD_COUNT);

    // 配置高性能服务器
    let addr = "0.0.0.0:50051".parse()?;
    println!("High-performance gRPC server listening on {}", addr);

    // 创建shutdown信号
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    // 启动服务器，使用 graceful shutdown
    let server_future = Server::builder()
        .add_service(grpc_service)
        .serve_with_shutdown(addr, async {
            shutdown_rx.await.ok();
        });

    // 等待 Ctrl+C 信号或服务器错误
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("\nReceived Ctrl+C, shutting down gracefully...");

            // 触发服务器关闭
            let _ = shutdown_tx.send(());

            // 关闭所有 channel，让处理器线程退出
            drop(sequencer_senders);
            drop(match_senders);
            drop(trade_execution_senders);
        }
        result = server_future => {
            if let Err(e) = result {
                eprintln!("Server error: {}", e);
            }
        }
    }

    // 等待处理器线程结束
    println!("Waiting for processors to finish...");
    for handle in processor_handles {
        let _ = handle.join();
    }

    // 等待撮合引擎线程结束
    for handle in match_handles {
        let _ = handle.join();
    }

    println!("Shutdown complete");
    Ok(())
}
