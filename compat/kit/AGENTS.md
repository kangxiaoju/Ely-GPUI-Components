# AGENTS.md: kit

Ely beside GPUI Kit.

## Commands

- `cargo test --manifest-path compat/kit/Cargo.toml --target-dir target/kit` runs Ely beside GPUI Kit in one window: Tab through both, each library's dialog over the other's fields, one asset source. The independent manifest pins Kit 0.9.1 with its `gpui-fast` feature and patches all nine Fast packages to `https://github.com/kangxiaoju/Ely-GPUI-Components.git` at revision `40427c396cc92b6946940e0e498612a8b680ce30`, matching Ely's Fast 0.1.14 backend source; `scripts/check.sh` resolves that pin rather than upgrading to the newest Kit release. These are Fast source overrides, not pre compatibility shims. Kit is not a product dependency. Its lockfile stays out of git.
