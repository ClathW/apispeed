# apispeed

A terminal UI for benchmarking LLM API streaming speed. Supports any OpenAI-compatible endpoint.

![demo](https://github.com/ClathW/apispeed/assets/demo.gif)

## Features

- Real-time streaming with live token throughput
- Metrics: TTFT, tokens/s, ms/token, p50/p90/p99 latency percentiles
- Keyboard and mouse scroll to review output
- Ctrl+C to cancel mid-stream

## Install

```bash
cargo install --git https://github.com/ClathW/apispeed
```

Or build from source:

```bash
git clone https://github.com/ClathW/apispeed
cd apispeed
cargo build --release
./target/release/apispeed
```

## Usage

Fill in the form fields:

| Field   | Description                                      |
|---------|--------------------------------------------------|
| URL     | API endpoint, e.g. `https://api.openai.com/v1/chat/completions` |
| API Key | Bearer token                                     |
| Model   | Model name, e.g. `gpt-4o`                        |
| Prompt  | The prompt to send                               |

### Keybindings

| Key              | Action              |
|------------------|---------------------|
| Tab / Shift+Tab  | Navigate fields     |
| Enter            | Start               |
| ↑ / k            | Scroll up           |
| ↓ / j            | Scroll down         |
| Page Up/Down     | Fast scroll         |
| Ctrl+C           | Cancel streaming    |
| Esc / q          | Back to form        |

## License

MIT
