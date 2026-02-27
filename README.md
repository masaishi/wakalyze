# wakalyze

Simple CLI to list [Wakapi](https://wakapi.dev) working hours per day using the heartbeats API.

## Output Example

```
$ wakalyze 2026/02 1

2026/02 week 1
- 2/1
  - 2:02pm ~ 2:31pm (0h28m) project-a
  - 2:56pm ~ 4:27pm (1h31m) project-a
  - 4:55pm ~ 5:17pm (0h22m) project-b
  - 7:50pm ~ 8:50pm (1h00m) project-a

- 2/2
  - 1:45am ~ 1:46am (0h00m) project-a
  - 4:44am ~ 5:27am (0h43m) project-b
```

## Install

### Homebrew (macOS / Linux)

```bash
brew install masaishi/wakalyze/wakalyze
```

### Cargo

Requires [Rust](https://www.rust-lang.org/tools/install).

```bash
cargo install wakalyze
```

### From source

```bash
git clone https://github.com/masaishi/wakalyze.git
cd wakalyze
cargo build --release
# binary at target/release/wakalyze
```

## Setup

Get your API key from Wakapi: Settings → API Key (e.g. https://wakapi.dev/settings#api_keys or `https://<your-wakapi-instance>/settings#api_keys`).

Store credentials in config:

```bash
wakalyze config set --key "<your-token>" --user "<your-username>"
wakalyze config set --base-url "https://wakapi.dev"
```

Or use environment variables:

```bash
export WAKAPI_KEY="<your-token>"
export WAKAPI_USER="<your-username>"
# optional
export WAKAPI_BASE_URL="https://wakapi.dev"
```

Config is saved at `~/.config/wakalyze/config.json` (or `$XDG_CONFIG_HOME/wakalyze/config.json`).

## Usage

```bash
wakalyze 2026/02
wakalyze 2026/02 1
wakalyze 2026/02 --filter "myproject"
wakalyze 2026/02 --filter "proj-a,proj-b"
wakalyze 2026/02 --max-gap-minutes 10
```

### Config management

```bash
wakalyze config show
wakalyze config path
wakalyze config set --user "<your-username>"
wakalyze config set --key "<your-token>"
wakalyze config set --base-url "https://wakapi.dev"
wakalyze config set --clear-key
```

### Resolution order

- `--user` / `--base-url` args
- `WAKAPI_USER` / `WAKAPI_BASE_URL` env vars
- stored config (`wakalyze config set`)
- default base url: `https://wakapi.dev`

Auth is resolved in this order:

- stored config key (`wakalyze config set --key`)
- `WAKAPI_KEY`

## Notes

- Time is estimated from heartbeat gaps (<= 15 minutes by default; adjust with `--max-gap-minutes`).
- Week numbers follow calendar weeks (Monday start). Week 1 is the row containing the 1st of the month, so it may include days from the previous month, and the last week may include days from the next month.
- The `--filter` flag supports comma-separated terms matched as OR (case-insensitive substrings).

## Development

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Acknowledgments

- [Wakapi](https://github.com/muety/wakapi) - Open-source WakaTime-compatible backend
- [WakaTime](https://wakatime.com) - Coding time tracking

## License

MIT
