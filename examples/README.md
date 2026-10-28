# Examples

Each example in this directory is a small crawl, documented at the
top of its `main.rs`:

| Command | What it shows |
| --- | --- |
| `cargo run -p basic-usage` | Tags, path routes, link following and `#[derive(Select)]` |
| `cargo run -p api-endpoints` | Typed JSON responses and following API pagination |
| `cargo run -p chrome-webdriver` | Browser automation with `Tab`; needs `chromedriver --port=4444` |
| `cargo run -p model-extraction` | Typed model extraction with a scripted provider, fully offline |
| `cargo run -p objective-crawl` | Shared objectives, relevant links and stopping, fully offline |
| `cargo run -p rust-blog` | Crawls the Rust blog through robots.txt, its sitemap and links into JSON Lines, resumable with redb |

`RUST_LOG=spire_core=debug` logs every request with its outcome and duration.
