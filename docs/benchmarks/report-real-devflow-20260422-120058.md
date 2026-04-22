# Priority Agent Benchmark Report

- label: `real-devflow`
- generated_at: `2026-04-22 12:01:00 +0800`
- runs_per_metric: `1`
- binary: `target/release/priority-agent`
- api_port: `18787`
- llm_key_detected: `1`

## Metrics

| metric | status | avg_ms | p95_ms | notes |
|---|---|---:|---:|---|
| startup_help | ok | 13.0 | 13 |  |
| startup_help raw runs | - | - | - | `13` |
| first_token | ok | 1241.0 | 1241 | POST /api/chat |
| first_token raw runs | - | - | - | `1241` |
| tool_call | ok | 24.0 | 24 | POST /api/tools/call (calculate) |
| tool_call raw runs | - | - | - | `24` |
| long_chat | skipped | N/A | N/A | disabled by default (use --enable-long-chat) |
| long_chat raw runs | - | - | - | `N/A` |

## Environment

```text
uname: Darwin GeorgedeMacBook-Air-2.local 25.4.0 Darwin Kernel Version 25.4.0: Thu Mar 19 19:32:36 PDT 2026; root:xnu-12377.101.15~1/RELEASE_ARM64_T8103 arm64
rustc: rustc 1.94.1 (e408947bf 2026-03-25)
cargo: cargo 1.94.1 (29ea6fb6a 2026-03-24)
```

## API Server Log Tail

```text
```
