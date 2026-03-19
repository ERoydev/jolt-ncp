# Illegal Device Store: 0x0

## Error

```
thread 'main' panicked at tracer/src/emulator/mmu.rs:198:13:
Illegal device store: Unknown memory mapping: 0x0
```

## What's happening

1. Guest binary uses **musl libc** (comes with Jolt's `riscv64imac-zero-linux-musl` target)
2. On boot, musl checks if stdin/stdout/stderr are open using `ppoll` (syscall 73) via inline ecall
3. **ZeroOS** (Jolt's built-in guest runtime, from LayerZero) **doesn't support `ppoll`** — returns `-ENOSYS`
4. musl treats this as fatal and hits a deliberate crash instruction (`sb zero, 0(zero)` = write to address 0x0)
5. Jolt's tracer MMU sees the write to 0x0 and panics

**ZeroOS is a transitive dependency of `jolt-sdk` (a16z/jolt) — we don't control it.**

## What doesn't work

- **`JoltMemory` workaround** — avoiding `std::io` in guest code does NOT eliminate the ppoll crash path. Other guest deps pull in enough musl code regardless. Tested and confirmed: crash still happens with `JoltMemory`.
- **Patching `ppoll.o` in musl's `libc.a`** — the ppoll call in the crash path is an **inline ecall** (direct syscall), not a function call to the `ppoll()` wrapper. Replacing the wrapper has no effect.

## Fix

Fork ZeroOS and add `ppoll` to the syscall registry. See [fix-ppoll-zeroos.md](./fix-ppoll-zeroos.md).

## Reproduce

```bash
cargo clean
CC_riscv64imac_zero_linux_musl="$HOME/.zeroos/musl/bin/riscv64-linux-musl-gcc" \
CFLAGS_riscv64imac_zero_linux_musl="-mcmodel=medany -mabi=lp64 -march=rv64imac" \
cargo run --release -- \
  --tx-hash 0x8e303e5bb591a69307f292a605e8780156c41eff978c200c307cde12f043c2d4 \
  --network mainnet
```
