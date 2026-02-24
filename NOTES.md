
1. 
Jolt custom RISC-V target spec uses "64" (string) for `target-pointer-width`, but with Rust 1.92 it expects a number.
Jolt requires Rust 1.88, Rust 1.92 broke compatibility with Jolt's custom target spec

2. Input size
- The input i try to give is like 1.1MB, but Jolt's default `max_input_size` is only 4096 bytes (4KB)