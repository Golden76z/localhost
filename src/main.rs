use std::env;
use std::process;

use localhost::config::Config;
use localhost::ServerEngine;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: {} <config.conf>", args[0]);
        process::exit(1);
    }

    let config = match Config::load(&args[1]) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("config error: {e}");
            process::exit(1);
        }
    };

    if config.servers.is_empty() {
        eprintln!("no valid server blocks loaded");
        process::exit(1);
    }

    eprintln!(
        "localhost: {} server block(s), {} listen socket(s)",
        config.servers.len(),
        config.listen_sockets().len()
    );

    if let Err(e) = ServerEngine::run(config) {
        eprintln!("server error: {e}");
        process::exit(1);
    }
}
