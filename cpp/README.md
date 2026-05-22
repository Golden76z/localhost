# C++ implementation (bonus)

Scaffold for the second-language bonus. Target parity with `configs/default.conf`:

- `epoll` event loop (or `kqueue` on BSD)
- Same config grammar
- Same audit behavior (one multiplex call per loop iteration)

Suggested layout:

```
cpp/
├── CMakeLists.txt
├── include/
└── src/
    ├── main.cpp
    ├── config.cpp
    ├── epoll_loop.cpp
    └── http.cpp
```

Build on Linux:

```bash
cmake -B build && cmake --build build
./build/localhost configs/default.conf
```

Run `tests/run_all.sh` against both binaries during audit prep.
