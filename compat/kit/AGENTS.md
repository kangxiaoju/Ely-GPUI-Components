# AGENTS.md: kit

Ely beside GPUI Kit.

## Commands

- `cargo test --manifest-path compat/kit/Cargo.toml --target-dir target/kit` runs Ely beside GPUI Kit in one window: Tab through both, each library's dialog over the other's fields, one asset source. The independent manifest pins Kit 0.9.1 with its `gpui-fast` feature and patches all nine Fast packages to `https://github.com/kangxiaoju/Ely-GPUI-Components.git` at revision `a0a941b84dbe0986575b11cb3c9025e177976c4b`, matching Ely's Fast 0.1.14 backend source; `scripts/check.sh` resolves that pin rather than upgrading to the newest Kit release. These are Fast source overrides, not pre compatibility shims. Kit is not a product dependency. Its lockfile stays out of git.
