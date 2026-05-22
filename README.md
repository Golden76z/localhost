# Localhost

A **single-process, single-thread** HTTP/1.1 server written in Rust, driven by **`epoll`** on Linux. Built for the 42 *Localhost* project and structured so you can walk an auditor through the code in minutes.

> **Platform:** **Audit / siege** must run on **Linux** (`epoll` in `src/net/epoll.rs`). **Windows** builds use a `std::net` poll backend for local dev (`src/net/poll_std.rs`) — run `cargo build` and `cargo test` on either OS.

## Quick start

```bash
cargo build --release
./target/release/localhost configs/default.conf
```

Open [http://127.0.0.1:8080](http://127.0.0.1:8080) in your browser.

### Live chat (browser — audit path)

[http://127.0.0.1:8080/chat.html](http://127.0.0.1:8080/chat.html) — POST messages, SSE stream (`/chat/api/send`, `/chat/api/events`).

### Operator GUI (optional, not audited)

```bash
cargo run --features gui --bin localhost-gui -- configs/default.conf
```

Shows HTTP log + chat mirror. Visitors still use the browser.

## Audit cheat sheet

| Topic | Where in code |
|-------|----------------|
| Event loop (one `epoll_wait` / iteration) | `src/net/epoll.rs`, `src/server/engine.rs` |
| One read **or** one write per client per wake | `handle_client_io()` in `engine.rs` |
| Non-blocking sockets | `src/net/socket.rs` |
| Config / virtual hosts | `src/config/` |
| GET / POST / DELETE | `src/server/handler.rs` |
| Cookies & sessions | `src/session/mod.rs` |
| CGI (fork + env) | `src/cgi/mod.rs`, `cgi-bin/` |
| Chunked bodies | `decode_chunked()` in `src/http/request.rs` |

## Stress test (your machine only)

```bash
siege -b -c25 -t1M http://127.0.0.1:8080/empty.html
```

Target: **≥ 99.5%** availability. Watch memory with `top` or Valgrind:

```bash
valgrind --leak-check=full ./target/release/localhost configs/default.conf
```

## Tests

```bash
chmod +x tests/*.sh
./tests/run_all.sh
```

## Bonus

- Second CGI: `.pl` in `configs/default.conf` → `cgi-bin/hello.pl`
- C++ twin: see `cpp/README.md` (scaffold; implement for full bonus)

## Docs

- [PLAN.md](PLAN.md) — phased roadmap vs audit checklist
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — diagrams and design narrative

## Fancy demo

The `www/` site is a dark-themed showcase: static pages, directory listing, upload form, session counter, and Python/Perl CGI demos.
