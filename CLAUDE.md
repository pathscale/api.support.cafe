@AGENTS.md

# Claude Code notes — api.support.cafe

The import above is binding: [`AGENTS.md`](AGENTS.md) is the **working agreement** for this
repository, and every Claude Code session loads it automatically. Don't copy rules here —
one source of truth, no drift. Only genuinely Claude-specific wiring belongs below.

- Guardrails live in [`.claude/settings.json`](.claude/settings.json): safe read-only cargo
  and git commands are pre-allowed; pushes, `cargo publish`, `gh pr merge`, cloud CLIs and
  deploys prompt first (`permissions.ask` plus the `PreToolUse` hook in
  [`.claude/hooks/`](.claude/hooks/)).
- Keep the hook's `RISKY_WORDS` and `permissions.ask` **in sync** — they back each other up.
