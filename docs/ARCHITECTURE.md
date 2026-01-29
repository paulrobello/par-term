# System Architecture

This document provides a high-level overview of the `par-term` architecture, detailing its core components, data flow, and rendering pipeline.

## Table of Contents
- [Overview](#overview)
- [High-Level Architecture](#high-level-architecture)
- [Core Components](#core-components)
  - [Application Logic](#application-logic)
  - [Terminal Emulation](#terminal-emulation)
  - [Rendering Engine](#rendering-engine)
  - [Text & Font Handling](#text--font-handling)
- [Data Flow](#data-flow)
- [Threading Model](#threading-model)
- [Related Documentation](#related-documentation)

## Overview

`par-term` is a GPU-accelerated, cross-platform terminal emulator frontend written in Rust. It leverages the [par-term-emu-core-rust](https://github.com/paulrobello/par-term-emu-core-rust) library for VT emulation and PTY management, while providing a modern rendering pipeline using `wgpu`.

**Key Architectural Goals:**
*   **Performance:** GPU-based rendering for high frame rates and low latency.
*   **Modularity:** Separation of concerns between UI, emulation, and rendering.
*   **Cross-Platform:** Native support for macOS (Metal), Windows (DirectX 12), and Linux (Vulkan/X11).
*   **Extensibility:** Support for custom shaders and advanced graphics protocols (Sixel, iTerm2, Kitty).

## High-Level Architecture

The system is composed of three primary layers: the Application Layer (handling OS events, state, and multi-tab management), the Emulation Layer (managing PTY sessions and VT state), and the Presentation Layer (rendering to the screen).

```mermaid
graph TB
    subgraph "Application Layer"
        App[App Entry Point]
        WM[Window Manager]
        WS[Window State]
        TabMgr[Tab Manager]
        TabUI[Tab Bar UI]
        Menu[Native Menu]
        Input[Input Handler]
        Config[Configuration]
    end

    subgraph "Emulation Layer"
        Tab[Tab / Terminal Session]
        TM[Terminal Manager]
        Core[Core Emulation Library]
        PTY[PTY Process]
    end

    subgraph "Presentation Layer"
        Renderer[Master Renderer]
        CellRender[Cell Renderer]
        GraphicRender[Graphics Renderer]
        Shader[Custom Shaders]
        GPU[WGPU / GPU]
    end

    App --> WM
    WM --> WS
    WM --> Menu
    Input --> WS
    WS --> TabMgr
    WS --> TabUI
    TabMgr --> Tab
    Tab --> TM
    WS --> Renderer
    TM --> Core
    Core <--> PTY
    Core --> Renderer
    Renderer --> CellRender
    Renderer --> GraphicRender
    CellRender --> Shader
    Shader --> GPU
    GraphicRender --> GPU

    style App fill:#e65100,stroke:#ff9800,stroke-width:3px,color:#ffffff
    style WM fill:#ff6f00,stroke:#ffa726,stroke-width:2px,color:#ffffff
    style WS fill:#ff6f00,stroke:#ffa726,stroke-width:2px,color:#ffffff
    style TabMgr fill:#ff6f00,stroke:#ffa726,stroke-width:2px,color:#ffffff
    style TabUI fill:#880e4f,stroke:#c2185b,stroke-width:2px,color:#ffffff
    style Menu fill:#880e4f,stroke:#c2185b,stroke-width:2px,color:#ffffff
    style Tab fill:#1b5e20,stroke:#4caf50,stroke-width:2px,color:#ffffff
    style TM fill:#1b5e20,stroke:#4caf50,stroke-width:2px,color:#ffffff
    style Renderer fill:#0d47a1,stroke:#2196f3,stroke-width:2px,color:#ffffff
    style PTY fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
    style GPU fill:#4a148c,stroke:#9c27b0,stroke-width:2px,color:#ffffff
```

## Core Components

### Application Logic

*   **App (`src/app/mod.rs`)**: The entry point that initializes configuration and runs the event loop via `winit`.
*   **WindowManager (`src/app/window_manager.rs`)**: Coordinates multiple terminal windows, handles native menu events, and manages shared resources.
*   **WindowState (`src/app/window_state.rs`)**: Per-window state containing tab manager, renderer, input handler, and UI components.
*   **TabManager (`src/tab/manager.rs`)**: Manages multiple terminal tabs within a window, handling tab creation, switching, reordering, and cleanup.
*   **Tab (`src/tab/mod.rs`)**: Represents a single terminal session with its own terminal, scroll state, mouse state, bell state, and render cache.
*   **TabBarUI (`src/tab_bar_ui.rs`)**: egui-based tab bar renderer with click handling, close buttons, activity indicators, and bell icons.
*   **Input Handler (`src/input.rs`)**: Translates OS window events (keyboard, mouse) into terminal input sequences or application commands (e.g., shortcuts for copy/paste).
*   **Menu (`src/menu/mod.rs`)**: Native cross-platform menu bar using `muda` (macOS global menu, Windows/Linux per-window menus).
*   **Configuration (`src/config/mod.rs`)**: Manages settings loaded from YAML files, handling platform-specific paths (`%APPDATA%` vs `~/.config`).
*   **Settings UI (`src/settings_ui/mod.rs`)**: egui-based settings overlay with tabs for font, theme, window, terminal, cursor, shell, bell, mouse, scrollbar, background, screenshot, and tab bar configuration.

### Terminal Emulation

*   **Terminal Manager (`src/terminal/mod.rs`)**: A wrapper around the core emulation library. It exposes a thread-safe API for the UI to interact with the underlying PTY session.
*   **Shell Spawning (`src/terminal/spawn.rs`)**: Handles shell process creation and login shell initialization.
*   **Graphics (`src/terminal/graphics.rs`)**: Manages Sixel and inline graphics metadata.
*   **Clipboard (`src/terminal/clipboard.rs`)**: Clipboard history and OSC 52 synchronization.
*   **Hyperlinks (`src/terminal/hyperlinks.rs`)**: OSC 8 hyperlink tracking and URL detection.
*   **Core Library**: Uses `par-term-emu-core-rust` for:
    *   VT100/ANSI escape sequence parsing.
    *   Grid management and scrollback history.
    *   PTY process lifecycle (spawning shell, resizing, I/O).

### Rendering Engine

*   **Renderer (`src/renderer/mod.rs`)**: The high-level rendering coordinator. It manages the `wgpu` surface and delegates tasks to specialized sub-renderers.
*   **Cell Renderer (`src/cell_renderer/mod.rs`)**: Responsible for drawing the text grid. Includes glyph atlas management (`atlas.rs`), background images (`background.rs`), and the core render loop (`render.rs`).
*   **Graphics Renderer (`src/graphics_renderer.rs`)**: Handles overlay graphics like Sixel, iTerm2 images, and Kitty graphics.
*   **Custom Shaders (`src/custom_shader_renderer/`)**: Provides post-processing effects using GLSL shaders (compatible with Shadertoy/Ghostty). Includes GLSL-to-WGSL transpilation via `naga`, channel texture management (`textures.rs`) for iChannel1-4 inputs, and uniform handling (`types.rs`).

### Text & Font Handling

*   **Font Manager (`src/font_manager/mod.rs`)**: Handles font discovery and fallback. It supports:
    *   **Primary Font**: The main user-configured monospace font.
    *   **Styled Variants**: Separate fonts for Bold, Italic, etc.
    *   **Range Fonts**: Specific fonts for Unicode ranges (e.g., CJK, Emoji).
    *   **Fallbacks**: System font fallback for missing glyphs.
*   **Text Shaper (`src/text_shaper.rs`)**: Uses `rustybuzz` (HarfBuzz) to shape text, handling ligatures, complex scripts, and combining characters correctly. Rasterization is performed by `swash`.

## Data Flow

The flow of data from user input to screen update is bidirectional.

```mermaid
sequenceDiagram
    participant User
    participant App as App/Input
    participant Term as Terminal
    participant PTY as PTY/Shell
    participant Render as Renderer
    participant Screen

    User->>App: Key Press
    App->>Term: Write Bytes
    Term->>PTY: Send to Stdin
    
    loop Async Processing
        PTY->>Term: Output (stdout/stderr)
        Term->>Term: Parse ANSI & Update Grid
    end

    App->>Term: Poll Updates
    Term-->>App: Grid State / Graphics
    App->>Render: Update State
    Render->>Screen: Draw Frame
```

1.  **Input**: User presses a key; `InputHandler` converts it to bytes.
2.  **Transmission**: Bytes are sent to the PTY via `TerminalManager`.
3.  **Processing**: The shell (e.g., zsh, bash) processes input and writes output.
4.  **Emulation**: The core library parses the output, updating the internal grid state.
5.  **Presentation**: The `App` polls for changes (or is notified) and triggers the `Renderer` to draw the new state to the `Screen`.

## Threading Model

`par-term` employs a hybrid threading model to ensure UI responsiveness.

*   **Main Thread**: Handles the OS event loop (`winit`), UI events, and rendering commands. This is critical as many OS windowing operations must occur on the main thread.
*   **Async Runtime (Tokio)**: A separate thread pool manages asynchronous tasks, primarily:
    *   Reading from and writing to the PTY.
    *   Handling timers (e.g., cursor blink, visual bell).
    *   Managing clipboard synchronization.

Access to shared resources (like the Terminal state) is managed via `parking_lot::Mutex` to prevent contention and ensure safety.

## Related Documentation

- [Documentation Style Guide](DOCUMENTATION_STYLE_GUIDE.md) - Standards for project documentation.
- [Compositor Architecture](COMPOSITOR.md) - Deep dive into the GPU rendering pipeline and shader system.
- [Custom Shaders Guide](CUSTOM_SHADERS.md) - Installing and creating custom GLSL shaders.
