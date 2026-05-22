//! Localhost — HTTP/1.1 server (single process, single thread, epoll).

pub mod cgi;
pub mod chat;
pub mod config;
pub mod events;
pub mod http;
pub mod net;
pub mod server;
pub mod session;

pub use config::Config;
pub use events::{EventSender, ServerEvent};
pub use server::ServerEngine;
