
1. 
Jolt custom RISC-V target spec uses "64" (string) for `target-pointer-width`, but with Rust 1.92 it expects a number.
Jolt requires Rust 1.88, Rust 1.92 broke compatibility with Jolt's custom target spec

ckb-vm-0.24.14 - requires rust 1.92

2. Input size
- The input i try to give is like 1.1MB, but Jolt's default `max_input_size` is only 4096 bytes (4KB)

3. `Illegal device store: Unknown memory mapping: 0x0` panic error:
- TODO

# Forks
Keep the `Cargo.lock` as it is, avoid using `cargo update` it may update other deps that the current jolt version used is dependent from and that may break stuff use `cargo update -p <specific_package>`

1. ZeroOs fork:
- Add a handler for `ppoll` syscall to `sys_noop`.