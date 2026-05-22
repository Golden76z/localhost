# Localhost — Implementation Plan

This document maps the **audit checklist** to concrete modules and phased delivery. The Rust server is the primary deliverable; C++ is the bonus second implementation.

## Design principles (audit talking points)

| Question | Answer |
|----------|--------|
| How does an HTTP server work? | Listen on TCP → accept connections → parse request line + headers + body → map URL to handler (static, redirect, CGI) → build response (status, headers, body) → send on same socket → close or keep-alive. |
| I/O multiplexing? | **`epoll`** on Linux (`EPOLLIN` / `EPOLLOUT`, edge-triggered). One `epoll_wait` per main-loop iteration; each ready fd gets **at most one** `read` or `write` before the next `epoll_wait`. |
| Why one select/epoll per cycle? | Avoids starvation and keeps the event loop fair under load (siege). Achieved with per-connection state machines and a `pending_io` flag. |
| Errors on sockets? | Checked on every syscall; broken clients are removed from epoll and closed. |
| All I/O through epoll? | Yes for clients and listeners; config file is read synchronously at startup (allowed by spec). |

## Repository layout

```
localhost/
├── src/                    # Rust server
│   ├── main.rs
│   ├── lib.rs
│   ├── config/             # Parse + validate .conf
│   ├── net/                # epoll, sockets, timers
│   ├── http/               # Parser, response builder
│   ├── server/             # Virtual hosts, routes, handlers
│   ├── session/            # Cookies + in-memory sessions
│   └── cgi/                # Fork + env + pipe I/O
├── cpp/                    # Bonus: second implementation (same conf format)
├── configs/                # Example configurations for audit
├── www/                    # Static site + custom error pages
├── cgi-bin/                # Python (+ optional Perl) scripts
└── tests/                  # Shell + integration tests
```

## Phases

### Phase 1 — Core event loop (DONE in scaffold)
- [x] Non-blocking listen/accept
- [x] `epoll` registry: listeners + clients
- [x] Request timeout (idle timer per connection)
- [x] One read OR one write per client per `epoll_wait`

### Phase 2 — HTTP + static (IN PROGRESS)
- [x] HTTP/1.1 request parser (chunked + Content-Length)
- [x] Response builder with correct status codes
- [x] Static files + directory index + optional autoindex
- [x] Default error pages: 400, 403, 404, 405, 413, 500
- [ ] Full route table from config (methods, redirect, root, index)

### Phase 3 — Methods & uploads
- [ ] GET / POST / DELETE with route method lists → 405
- [ ] POST multipart and raw body → save under upload dir
- [ ] `client_max_body_size` → 413

### Phase 4 — Virtual hosts & config hardening
- [ ] `server_name` + default server per host:port
- [ ] `curl --resolve` hostname routing
- [ ] Duplicate port detection (fatal for that block, others still load)
- [ ] Invalid server block skipped; rest of config keeps running

### Phase 5 — Sessions & CGI
- [ ] `Set-Cookie` / `Cookie` session id (HttpOnly)
- [ ] CGI: fork, env vars, stdin body, capture stdout → HTTP response
- [ ] Python CGI + **bonus** Perl or second interpreter
- [ ] Chunked POST to CGI tested in `tests/cgi.sh`

### Phase 6 — Polish & audit kit
- [ ] `siege -b` runbook in README (≥99.5% availability)
- [ ] Valgrind / massif notes for leaks
- [ ] Browser demo site under `www/` (fancy landing + forms + upload UI)
- [ ] Exhaustive `tests/` scripts

### Phase 7 — Bonus C++
- [ ] `cpp/` server: same `configs/default.conf`, epoll/select, parity tests

## Config grammar (implemented)

```
server {
    listen 127.0.0.1:8080;
    server_name localhost;
    root www;
    client_max_body_size 10M;
    error_page 404 /errors/404.html;

    route / {
        methods GET POST;
        index index.html;
        autoindex off;
    }

    route /upload {
        methods POST;
        upload_dir uploads;
    }

    route /old {
        redirect https://example.com/new;
    }

    route /cgi {
        cgi .py /usr/bin/python3;
        cgi .pl /usr/bin/perl;    # bonus
    }
}
```

## Audit demo script (quick reference)

```bash
# Build (Linux / WSL)
cargo build --release

# Single / multi port
./target/release/localhost configs/single.conf
./target/release/localhost configs/multi_port.conf

# Hostname routing
curl --resolve test.com:8080:127.0.0.1 http://test.com:8080/

# Body limit
curl -X POST -d "$(python3 -c 'print("x"*20000000)')" http://127.0.0.1:8080/upload

# Siege (only your machine!)
siege -b -c10 -t30s http://127.0.0.1:8080/empty.html

# Tests
./tests/run_all.sh
```

## What to show off (“fancy”)

- Polished `www/` with dark theme, live request log via CGI, session dashboard, drag-and-drop upload
- `configs/audit.conf` with comments pointing to each audit bullet
- `docs/ARCHITECTURE.md` with sequence diagram (browser → epoll → handler)
- Clean Rust modules auditors can navigate in 5 minutes
