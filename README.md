# oxidlp

A terminal-based YouTube video downloader built in Rust. This project serves as both a functional tool and a learning exercise in building responsive, async TUI applications us ing RUST

## Why Another YouTube Downloader?

Because clicking through browser extensions is for people with patience. This is for those who prefer typing commands and watching progress bars in the terminal.

More seriously: this project explores how to build a proper async TUI application in Rust, handling concurrent downloads, real-time progress updates, and user input without blocking the main thread.

---

## Technical Overview

### Architecture

The application follows an **event-driven, async worker architecture** with clear separation of concerns:

```
┌─────────────────────────────────────────────────────────────┐
│                      TUI Layer (ratatui)                    │
│  Renders UI at ~60fps, handles keyboard input               │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                  Event & State Controller                   │
│  Central App struct, event dispatch, state management       │
└─────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              ▼                               ▼
┌─────────────────────────┐     ┌─────────────────────────────┐
│    Async Worker Pool    │     │      External Process       │
│  Bounded concurrency    │────▶│      yt-dlp subprocess      │
│  via Semaphore          │     │      stdout streaming       │
└─────────────────────────┘     └─────────────────────────────┘
```

### Core Components

| Module | Responsibility |
|--------|----------------|
| `main.rs` | Entry point, terminal setup, main event loop |
| `app.rs` | Application state, event handling, business logic |
| `events.rs` | Event types, job states, worker commands |
| `config.rs` | Configuration loading/saving, yt-dlp availability check |
| `ui/mod.rs` | All rendering logic using ratatui |
| `ui/input.rs` | Keyboard input handling and event dispatch |
| `worker/mod.rs` | Worker pool with bounded concurrency |
| `worker/ytdlp.rs` | yt-dlp process management, progress parsing |

### Key Design Decisions

#### 1. Bounded Concurrency with Semaphore

Downloads are limited by a configurable `max_concurrent_downloads` value (default: 3). This uses `tokio::sync::Semaphore` to acquire permits before spawning download tasks:

```rust
let semaphore = Arc::new(tokio::sync::Semaphore::new(config.max_concurrent_downloads));

// Before each download:
let permit = semaphore.clone().acquire_owned().await?;
// Download runs...
// Permit dropped automatically when task completes
```

#### 2. Message Passing Over Shared State

The application uses `tokio::sync::mpsc` channels for communication between components:

- `AppEvent` channel: Workers send progress updates, completion/failure events
- `WorkerCommand` channel: UI sends download requests, cancellation signals

This avoids complex locking scenarios and makes the data flow explicit.

#### 3. Graceful Cancellation

Each download task receives a `CancellationToken` that can be triggered from the UI. The download loop checks for cancellation within its `tokio::select!`:

```rust
tokio::select! {
    _ = cancel.cancelled() => {
        child.kill().await?;
        return Err(anyhow!("Download cancelled"));
    }
    result = reader.next_line() => {
        // Process progress update
    }
}
```

#### 4. Zero-Copy Progress Streaming

Instead of buffering yt-dlp's entire output, we stream stdout line-by-line using `AsyncBufReadExt::next_line()`. Progress updates are parsed and sent over the channel immediately, keeping memory usage constant regardless of download duration.

#### 5. Config Wrapped in Arc

`Config` is wrapped in `Arc<Config>` and shared across worker tasks. Since config is read-only after startup, this avoids cloning the entire struct for each download:

```rust
let config = Arc::new(Config::load()?);
// Cheap reference count increment instead of deep clone
```

### State Machine

Each download job progresses through a well-defined state machine:

```
[FetchingFormats] ──── success ────▶ [Ready]
        │                              │
     failure                      (user selects format)
        ▼                              ▼
    [Failed] ◀──── failure ──── [Downloading] ──── success ────▶ [Completed]
                                       │
                                  (user cancels)
                                       ▼
                                  [Cancelled]
```

### UI Features

- **Two-panel layout**: Download queue on left, details on right
- **Format selection popup**: Choose video/audio quality per item
- **Settings popup**: Adjust concurrent downloads and output directory
- **System info panel**: CPU usage, memory RSS
- **Playlist detection**: Automatically expands YouTube playlists into individual jobs

### Performance Considerations

1. **Box<Job> in enum variants**: Large `Job` struct is boxed inside `WorkerCommand::StartJob` to reduce enum size from ~392 bytes to ~56 bytes
2. **Lazy format filtering**: Single-pass iteration instead of multiple filter chains
3. **String capacity pre-allocation**: Progress bar strings use `String::with_capacity()`
4. **Conditional sysinfo refresh**: Only refreshes process info when the system panel is visible

---

## Installation

### Prerequisites

- Rust 1.70+ (uses edition 2021)
- [yt-dlp](https://github.com/yt-dlp/yt-dlp) installed and in your PATH

### Build

```bash
git clone https://github.com/yourusername/oxidlp.git
cd oxidlp
cargo build --release
```

The binary will be at `target/release/oxidlp`.

### Release Profile

The release build is optimized for size and performance:

```toml
[profile.release]
lto = true           # Link-time optimization
codegen-units = 1    # Single codegen unit for better optimization
strip = true         # Strip symbols
panic = "abort"      # No unwinding for smaller binary
```

---

## Usage

```bash
cargo run --release
```

Or if you've built the binary:

```bash
./target/release/oxidlp
```

The TUI will launch and you can paste YouTube URLs directly into the input field.
### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Tab` | Switch between input and queue modes |
| `j/k` or `Arrow keys` | Navigate queue |
| `Enter` | Open format selector (on ready items) |
| `s` | Start all queued downloads |
| `d` | Remove selected item |
| `c` | Cancel active download |
| `g` | Open settings |
| `S` | Toggle system info panel |
| `?` | Show help |
| `q` | Quit (prompts if downloads active) |

### Configuration

Config file location: `~/.config/oxidlp/oxidlp/config.toml`

```toml
output_dir = "/home/user/Videos"
output_template = "%(title)s.%(ext)s"
max_concurrent_downloads = 3
default_format = "bestvideo+bestaudio/best"
```

---

## Dependencies

| Crate | Purpose |
|-------|---------|
| `ratatui` | Terminal UI framework |
| `crossterm` | Cross-platform terminal manipulation |
| `tokio` | Async runtime |
| `serde` + `toml` | Configuration serialization |
| `clap` | CLI argument parsing |
| `color-eyre` | Error handling with context |
| `tracing` | Structured logging |
| `sysinfo` | Process CPU/memory monitoring |

---

## Project Structure

```
oxidlp/
├── Cargo.toml
├── src/
│   ├── main.rs           # Entry point
│   ├── app.rs            # Application state
│   ├── events.rs         # Event types and data structures
│   ├── config.rs         # Configuration management
│   ├── ui/
│   │   ├── mod.rs        # Rendering logic
│   │   └── input.rs      # Input handling
│   └── worker/
│       ├── mod.rs        # Worker pool
│       └── ytdlp.rs      # yt-dlp integration
└── README.md
```

---

## What I Learned

Building this project provided hands-on experience with:

1. **Async Rust patterns**: Properly structuring concurrent code without data races
2. **Message passing**: Using channels instead of shared mutable state
3. **Process management**: Spawning external processes and streaming their output
4. **TUI development**: Building responsive interfaces that don't block on I/O
5. **State machines**: Modeling complex workflows as explicit state transitions
6. **Memory optimization**: Reducing allocations in hot paths

The hardest part was getting the cancellation right. It turns out that killing a child process is simple; making sure that your select! loop actually awaits the right things is where the fun begins.

---

## License

MIT

---

## Acknowledgments

- [yt-dlp](https://github.com/yt-dlp/yt-dlp) - The actual hero here. This project is just a fancy wrapper.
- [ratatui](https://github.com/ratatui-org/ratatui) - For making terminal UIs almost enjoyable to build.