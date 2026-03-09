# AGENTS.md

This project provides `nudge-me`, a thin wrapper around `codex cli`, `copilot cli`, and `claude code`.

Its main purpose is to detect when the underlying tool has been idle for a while and notify the user, so long-running interactive sessions do not silently stall.

## Glossary

- `idle overlay`: the centered visual notification shown when the wrapped tool has been idle for a while; it disappears on user input or resumed meaningful output.
