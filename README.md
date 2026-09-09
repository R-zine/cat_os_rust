# cat_os

`cat_os` is a small x86_64 kernel written in Rust. It boots through the
`bootloader` crate, writes to the VGA text buffer, handles CPU/PIC interrupts,
provides a heap allocator, and runs asynchronous tasks for keyboard input.

## Prerequisites

- Rustup (the pinned nightly toolchain and components are declared in
  `rust-toolchain.toml`)
- [`bootimage`](https://github.com/rust-osdev/bootimage)
- QEMU with `qemu-system-x86_64` available on `PATH`

Install the runner with:

```text
cargo install bootimage
```

## Build and run

Commands can be run from the repository root:

```text
cargo build
cargo run -p cat_os
```

## Tests and quality checks

The test runner boots each test image in QEMU and uses the ISA debug-exit
device to report success or failure:

```text
cargo test --workspace --all-features --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

The tests cover booting, VGA output, breakpoint and double-fault handling,
heap allocation, expected panics, and executor wake coalescing.

## Pre-commit hook

Enable the tracked hook once after cloning:

```text
git config core.hooksPath .githooks
```

Every commit will then run the formatting check, Clippy across all targets and
features, and the complete QEMU-backed test suite. `RUSTFLAGS` and
`RUSTDOCFLAGS` are set to `-D warnings`, so compiler, Clippy, test, and rustdoc
warnings all reject the commit.
