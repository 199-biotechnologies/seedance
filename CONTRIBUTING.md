# Contributing

Thanks for thinking about it. Three steps:

1. **Open an issue first** if the change is non-trivial. Saves both of us time if the approach needs a nudge.
2. **Fork, branch, commit.** Keep commits tight and describe the *why* in the message. Run `cargo test` before pushing.
3. **Open a PR** against `main`. CI runs on Ubuntu and macOS; green CI is required.

## Dev setup

```bash
git clone https://github.com/paperfoot/seedance-cli
cd seedance-cli
cargo build
cargo test
export SEEDANCE_API_KEY=sk-...
./target/debug/seedance doctor
```

## What we care about

- **Agent-friendliness.** Output shape on pipe must stay stable. Every user-visible error maps to a semantic exit code.
- **One binary.** No Python helpers, no shell wrappers. If you can't `cargo install` it, it doesn't ship.
- **Real users hit this.** If your change could break someone's script, flag it in the PR.

## Reporting bugs

Include the full command, `seedance --version`, and the output with `--json`. Redact your API key.
