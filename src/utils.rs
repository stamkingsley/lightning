use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use uuid::Uuid;

// 对象池 - 重用对象避免分配开销
pub struct ObjectPool<T> {
    objects: Arc<Mutex<VecDeque<T>>>,
    factory: Arc<dyn Fn() -> T + Send + Sync>,
    max_size: usize,
    current_size: AtomicUsize,
}

impl<T> ObjectPool<T> {
    pub fn new<F>(factory: F, initial_size: usize, max_size: usize) -> Self 
    where 
        F: Fn() -> T + Send + Sync + 'static 
    {
        let mut objects = VecDeque::new();
        for _ in 0..initial_size {
            objects.push_back(factory());
        }
        
        Self {
            objects: Arc::new(Mutex::new(objects)),
            factory: Arc::new(factory),
            max_size,
            current_size: AtomicUsize::new(initial_size),
        }
    }
    
    pub fn get(&self) -> T {
        if let Some(obj) = self.objects.lock().unwrap().pop_front() {
            self.current_size.fetch_sub(1, Ordering::Relaxed);
            obj
        } else {
            // 池空了，创建新对象
            (self.factory)()
        }
    }
    
    pub fn return_object(&self, mut obj: T) 
    where 
        T: Default 
    {
        // 重置对象状态
        obj = Default::default();
        
        if self.current_size.load(Ordering::Relaxed) < self.max_size {
            self.objects.lock().unwrap().push_back(obj);
            self.current_size.fetch_add(1, Ordering::Relaxed);
        }
        // 如果池满了，丢弃对象
    }
}

// UUID池 - 预生成UUID避免运行时生成开销
pub struct UuidPool {
    uuids: Arc<Mutex<VecDeque<Uuid>>>,
    generator_thread: std::thread::JoinHandle<()>,
    should_stop: Arc<std::sync::atomic::AtomicBool>,
}

impl UuidPool {
    pub fn new(pool_size: usize) -> Self {
        let uuids = Arc::new(Mutex::new(VecDeque::new()));
        let should_stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        
        // 预生成UUID
        for _ in 0..pool_size {
            uuids.lock().unwrap().push_back(Uuid::new_v4());
        }
        
        let uuids_clone = uuids.clone();
        let should_stop_clone = should_stop.clone();
        
        // 后台线程持续生成UUID
        let generator_thread = std::thread::spawn(move || {
            while !should_stop_clone.load(Ordering::Relaxed) {
                let mut uuids = uuids_clone.lock().unwrap();
                if uuids.len() < pool_size * 2 { // 保持2倍池大小
                    uuids.push_back(Uuid::new_v4());
                }
                drop(uuids);
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        });
        
        Self {
            uuids,
            generator_thread,
            should_stop,
        }
    }
    
    pub fn get_uuid(&self) -> Uuid {
        self.uuids.lock().unwrap().pop_front().unwrap_or_else(Uuid::new_v4)
    }
    
    pub fn return_uuid(&self, uuid: Uuid) {
        let mut uuids = self.uuids.lock().unwrap();
        if uuids.len() < 10000 { // 限制池大小
            uuids.push_back(uuid);
        }
    }
}

impl Drop for UuidPool {
    fn drop(&mut self) {
        self.should_stop.store(true, Ordering::Relaxed);
        let _ = std::mem::replace(&mut self.generator_thread, std::thread::spawn(|| {})).join();
    }
}

// 字符串池 - 重用常用字符串
pub struct StringPool {
    strings: Arc<Mutex<VecDeque<String>>>,
    max_size: usize,
}

impl StringPool {
    pub fn new(max_size: usize) -> Self {
        Self {
            strings: Arc::new(Mutex::new(VecDeque::new())),
            max_size,
        }
    }
    
    pub fn get_string(&self) -> String {
        self.strings.lock().unwrap().pop_front().unwrap_or_default()
    }
    
    pub fn return_string(&self, mut s: String) {
        s.clear(); // 清空内容但保留容量
        let mut strings = self.strings.lock().unwrap();
        if strings.len() < self.max_size {
            strings.push_back(s);
        }
    }
}

// 高性能消息处理器 - 使用对象池
pub struct MessageProcessor {
    receiver: crossbeam_channel::Receiver<crate::grpc::AsyncBalanceMessage>,
    balance_manager: crate::balance::BalanceManager,
    uuid_pool: UuidPool,
    string_pool: StringPool,
}

impl MessageProcessor {
    pub fn new(receiver: crossbeam_channel::Receiver<crate::grpc::AsyncBalanceMessage>) -> Self {
        Self {
            receiver,
            balance_manager: crate::balance::BalanceManager::new(),
            uuid_pool: UuidPool::new(10000),
            string_pool: StringPool::new(1000),
        }
    }
    
    pub fn run(mut self) {
        println!("High performance message processor started");
        loop {
            match self.receiver.recv() {
                Ok(message) => {
                    // 使用池化的UUID和字符串
                    self.process_message_with_pools(message);
                }
                Err(_) => {
                    println!("Channel closed, stopping high performance processor");
                    break;
                }
            }
        }
    }
    
    fn process_message_with_pools(&mut self, message: crate::grpc::AsyncBalanceMessage) {
        match message {
            crate::grpc::AsyncBalanceMessage::GetAccount { request_id: _, account_id, currency_id, response_sender } => {
                let response = self.balance_manager.handle_get_account(account_id, currency_id);
                let _ = response_sender.send(response);
            }
            crate::grpc::AsyncBalanceMessage::Increase { request_id: _, account_id, currency_id, amount, response_sender } => {
                // 重用字符串避免分配
                // let mut amount_str = self.string_pool.get_string();
                // amount_str.push_str(&amount);
                
                let response = self.balance_manager.handle_increase(account_id, currency_id, &amount);
                
                // 返回字符串到池中
                // self.string_pool.return_string(amount_str);
                let _ = response_sender.send(response);
            }
            crate::grpc::AsyncBalanceMessage::Decrease { request_id: _, account_id, currency_id, amount, response_sender } => {
                // let mut amount_str = self.string_pool.get_string();
                // amount_str.push_str(&amount);
                
                let response = self.balance_manager.handle_decrease(account_id, currency_id, &amount);
                
                // self.string_pool.return_string(amount_str);
                let _ = response_sender.send(response);
            }
        }
    }
}
