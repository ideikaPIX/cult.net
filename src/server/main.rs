#[path = "../shared.rs"]
pub mod shared;

mod network;
mod registry;
mod queue;
mod auth;

use std::env;

#[tokio::main]
async fn main() {
    let registry = registry::new_registry();
    let queue = queue::new_queue();
    let addr = env::var("CULT_SERVER_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    
    println!("CULT.NET Server starting on {}...", addr);
    network::start_server(&addr, registry, queue).await;
}
