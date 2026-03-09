# 2026-03-09 04:35 (UTC) : Public repo rename to nudge-me

Rebranded the project for public release as `nudge-me`, updated the CLI/binary name, and rewrote the README around the public-facing workflow.

# 2026-03-09 02:44 (UTC) : Softer card flash timing

Adjusted the `card` overlay timing so it flashes more slowly and then becomes steady.

Changes:
- `card` now flips color every 5 seconds instead of on the poll cadence
- after 1 minute, `card` stops flashing and stays on the blue state
- behavior remains time-based and independent of poll jitter

Verification:
- `cargo test`

# 2026-03-09 02:38 (UTC) : Card waits for user idle

Updated the `card` idle overlay so it waits for user inactivity as well as tool stall, and made the CLI threshold default to 30 seconds.

Changes:
- `card` overlay now appears only when the wrapped tool is stalled and the user has also been idle for the configured threshold
- `--threshold` now defaults to `30`
- Kept `zzz` behavior unchanged

Verification:
- `cargo test`

# 2026-03-09 02:24 (UTC) : Named idle overlay styles

Added named idle overlay styles so the notification can be selected by CLI option instead of being hardcoded.

Changes:
- Added `--idle-overlay <card|zzz>` with `card` as the default
- Restored the original card overlay as one renderer and added a moving `zzz` renderer
- Kept overlay selection isolated in the UI/controller path so new styles can be added without touching relay logic

Verification:
- `cargo test`

# 2026-03-09 01:31 (UTC) : Robust idle overlay

Added a centered blinking idle overlay that appears after stall detection and disappears on user input or resumed meaningful output.

Changes:
- Added a VT100 shadow-screen controller so the overlay can be shown and removed without corrupting the underlying TUI
- Split overlay logic into testable modules (`overlay`, `ui`) and made `stall` emit explicit start/resume transitions
- Wired terminal resize handling into the overlay renderer

Verification:
- `cargo test`

# 2026-03-05 04:46 (UTC) : Initial release

First prototype of pty-trap — a PTY wrapper with stall detection.

Features:
- Transparent PTY proxy: raw mode, SIGWINCH propagation, signal forwarding
- ANSI escape sequence stripper (CSI, OSC, control chars)
- Meaningful vs noise classifier with spinner/dot/progress-bar detection
- Stall state machine: logs `HH:MM:SS stop`/`move` to `notify.log`
- CLI: `--threshold`, `--notify-log`, child command after `--`

Limitations:
- File-based logging only (no HTTP/UDP webhook yet)
- Noise patterns not configurable via CLI
- No SIGTSTP (suspend/resume) handling
