# Mutex Patterns in par-term

par-term uses two distinct mutex types that serve different purposes. Choosing the wrong
one — or calling the wrong access method — is a source of deadlocks and panics. This
document explains the design, decision rules, and correct usage patterns.

---

## Why Two Mutex Types?

### The Boundary Problem

par-term straddles two execution environments:

| Environment | Driver |
|---|---|
| **Async tasks** | Tokio runtime (`Arc<Runtime>`) — spawned for PTY I/O, mouse reporting, key-send operations |
| **Sync event loop** | winit `EventLoop` — runs on the main thread, drives rendering and OS events |

A `tokio::sync::Mutex` can be held across `.await` points, which is essential for async
tasks. However, locking it from a **non-async context** requires `blocking_lock()`, which
parks the calling thread. Calling `blocking_lock()` from inside a Tokio worker thread will
deadlock if all worker threads are occupied.

A `parking_lot::Mutex` is a plain OS-level mutex. It has no async support but is smaller,
faster, and cannot cause Tokio deadlocks. It is the right choice when all callers are
sync threads.

---

## Decision Matrix

| State is shared with async tasks? | Use |
|---|---|
| Yes (PTY reader, input sender, resize, etc.) | `tokio::sync::Mutex` |
| No (only sync threads / event loop) | `parking_lot::Mutex` |

A secondary heuristic: if the critical section must be held across an `.await` point, the
type **must** be `tokio::sync::Mutex`.

---

## Key Types and Their Mutex Choices

### `tokio::sync::Mutex`

| Type | Field | Reason |
|---|---|---|
| `Tab` | `terminal: Arc<tokio::sync::Mutex<TerminalManager>>` | Shared with async PTY reader and input tasks |
| `Pane` | `terminal: Arc<tokio::sync::Mutex<TerminalManager>>` | Same — each pane has its own PTY |
| `AgentState` | `agent: Option<Arc<tokio::sync::Mutex<Agent>>>` | Accessed from spawned async prompt tasks |

### `parking_lot::Mutex`

| Type | Field | Reason |
|---|---|---|
| `SharedSessionLogger` (type alias) | `Arc<parking_lot::Mutex<Option<SessionLogger>>>` | Only accessed from sync event loop and std threads |
| `SystemMonitor` | `data: Arc<parking_lot::Mutex<SystemMonitorData>>` | Background std thread writer, sync render-thread reader |
| `StatusBarUI` | `status: Arc<parking_lot::Mutex<GitStatus>>` | Sync git-check thread + sync render thread |
| `DebugLogger` (static) | `OnceLock<parking_lot::Mutex<DebugLogger>>` | Non-async log writes from any thread |
| `ShaderWatcher` | `Arc<parking_lot::Mutex<HashMap<...>>>` | std thread writer, sync event loop reader |
| `AudioBell` | `sink: Option<Arc<parking_lot::Mutex<Player>>>` | Rodio plays on a std thread |
| `BadgeState` | `variables: Arc<parking_lot::RwLock<SessionVariables>>` | RwLock for frequent reads, infrequent writes, all sync |
| `FileTransferManager` | `error: Arc<parking_lot::Mutex<Option<String>>>` | Error state written from std thread, read from event loop |

---

## Accessing `Tab.terminal` from a Sync Context

`Tab.terminal` is `Arc<tokio::sync::Mutex<TerminalManager>>`. The winit event loop is
not an async context, so `.lock().await` is unavailable. Two alternatives exist:

### `try_lock()` — Non-blocking poll

```rust
// In the winit event loop (about_to_wait, window events, etc.)
if let Ok(term) = tab.terminal.try_lock() {
    // use term ...
} else {
    // Lock is held by an async task; skip this frame, retry next.
    crate::debug::record_try_lock_failure("resize");
}
```

Use `try_lock()` for:
- Per-frame rendering polls
- Resize propagation
- Any operation that can safely be deferred to the next frame

### `blocking_lock()` — Blocking wait

```rust
// For infrequent user-initiated operations (coprocess start/stop, scripting setup)
let term = tab.terminal.blocking_lock();
term.start_coprocess(...);
```

Use `blocking_lock()` for:
- Coprocess start / stop (user action, happens once)
- Scripting observer registration
- File transfer initiation triggered by user
- Any operation that **must not** be skipped

**WARNING**: Never call `blocking_lock()` from within a `runtime.spawn()`'d async task.
If all Tokio worker threads are waiting on `blocking_lock()`, Tokio will deadlock.

---

## Accessing `Tab.terminal` from an Async Context

```rust
// Inside runtime.spawn() or an async fn
let term = terminal_clone.lock().await;
// term is held across the await — safe with tokio::sync::Mutex
```

Never use `try_lock()` from async code to guard long-lived operations; prefer
`.lock().await` so the task yields instead of spinning.

---

## Anti-Patterns to Avoid

### Deadlock: `blocking_lock` inside a Tokio task

```rust
// BAD — never do this inside runtime.spawn()
runtime.spawn(async move {
    let term = terminal.blocking_lock(); // may deadlock Tokio thread pool
});

// GOOD
runtime.spawn(async move {
    let term = terminal.lock().await;
});
```

### Wrong mutex for a new async-shared type

```rust
// BAD — parking_lot::Mutex cannot be held across .await
let mu = Arc::new(parking_lot::Mutex::new(state));
runtime.spawn(async move {
    let guard = mu.lock(); // guard held, then ...
    some_async_fn().await; // ... held across await point: undefined behavior / panic
});

// GOOD
let mu = Arc::new(tokio::sync::Mutex::new(state));
runtime.spawn(async move {
    let guard = mu.lock().await;
    some_async_fn().await; // safe
});
```

### Calling `lock().await` from a sync context

```rust
// BAD — sync context, cannot .await
fn handle_event(&mut self) {
    let term = self.tab.terminal.lock().await; // compile error
}

// GOOD — use try_lock() or blocking_lock() depending on requirements
fn handle_event(&mut self) {
    if let Ok(term) = self.tab.terminal.try_lock() { ... }
}
```

### Using `tokio::sync::Mutex` for pure-sync state

```rust
// UNNECESSARY — if no async task ever touches this, parking_lot is simpler
let log: Arc<tokio::sync::Mutex<Logger>> = Arc::new(tokio::sync::Mutex::new(Logger::new()));

// BETTER
let log: Arc<parking_lot::Mutex<Logger>> = Arc::new(parking_lot::Mutex::new(Logger::new()));
```

---

## `try_lock()` Failure Telemetry

The codebase tracks `try_lock()` misses via `crate::debug::record_try_lock_failure(site)`.
This increments a global atomic counter and emits a `CONCURRENCY` debug log entry
(visible at `DEBUG_LEVEL >= 3`). Periodic summaries are emitted by `about_to_wait`.

When adding a new `try_lock()` call site, pass a short label:

```rust
if let Ok(term) = self.terminal.try_lock() {
    // ...
} else {
    crate::debug::record_try_lock_failure("my_operation");
}
```

A high miss rate at a specific site indicates that an async task is holding the lock
longer than expected, which may warrant investigation.

---

## Summary

```
tokio::sync::Mutex   — async tasks share the value; use .lock().await
                       sync callers: try_lock() (skip-able) or blocking_lock() (must-succeed)

parking_lot::Mutex   — all callers are sync threads; use .lock() directly
                       never hold across .await
```

See `CLAUDE.md` and `src/debug.rs` for additional context on the try_lock telemetry system.
