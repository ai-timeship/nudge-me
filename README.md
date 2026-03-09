# nudge-me

`nudge-me` wraps an interactive terminal tool, watches for meaningful output, and shows an idle overlay when the session appears stalled.

## What it does

- runs a child command inside a PTY
- ignores spinner/noise output when tracking activity
- logs `stop` / `move` transitions to a local event log
- shows a dismissible idle overlay until input or useful output resumes

## Usage

```bash
nudge-me [--threshold <seconds>] [--notify-log <path>] [--idle-overlay <card|zzz>] -- <command> [args...]
```

Examples:

```bash
nudge-me -- codex
nudge-me -t 10 --notify-log /tmp/nudge.log -- claude
nudge-me --idle-overlay zzz -- copilot
```

## Options

- `-t`, `--threshold`: idle threshold in seconds; default `30`
- `--notify-log`: event log path; default `./nudge.log`
- `--idle-overlay`: overlay style; `card` or `zzz`

## Build

```bash
cargo build --release
cp target/release/nudge-me ~/bin/
```

## Test

```bash
cargo test
```
