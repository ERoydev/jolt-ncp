# Fix: Add ppoll stub to ZeroOS

## Problem

Guest binary crashes at boot because musl's stdio init calls `ppoll` (syscall 73) and ZeroOS doesn't support it. Returns `-ENOSYS` → musl hits crash trap → `sb zero, 0(zero)` → Jolt panics with `Illegal device store: Unknown memory mapping: 0x0`.

## Why patching libc.a doesn't work

The `ppoll` call in the crash path is an **inline ecall** (direct syscall), not a function call to `ppoll()`. Replacing `ppoll.o` in musl's `libc.a` only replaces the wrapper function — the inlined ecall in musl's boot code is untouched.

## Fix: Fork ZeroOS and add ppoll handler

ZeroOS already has a `sys_noop` handler that returns 0. We just need to register it for syscall 73.

### Step 1: Fork ZeroOS

Fork `https://github.com/LayerZero-Labs/ZeroOS.git`

### Step 2: Add ppoll to syscall registry

File: `crates/zeroos-os-linux/src/syscall.rs`

In the `sys_registry!` macro, add:

```rust
(SYS_ppoll, handlers::sys_noop, 5),
```

`sys_noop` already exists in `handlers/mod.rs`:

```rust
#[inline]
pub fn sys_noop() -> isize {
    0
}
```

Returning 0 tells musl "all fds are ready" → stdio init completes → no crash.

### Step 3: Fork jolt-sdk

Fork `https://github.com/a16z/jolt`

Update the ZeroOS dependency to point to your ZeroOS fork. In jolt's `Cargo.toml`, change the `zeroos` git URL and rev to your fork's commit.

### Step 4: Point your project at forked jolt-sdk

In `Cargo.toml`:

```toml
jolt-sdk = { git = "https://github.com/<your-org>/jolt", rev = "<your-commit>", features = ["host"] }
```

In `guest/Cargo.toml`:

```toml
jolt = { package = "jolt-sdk", git = "https://github.com/<your-org>/jolt", rev = "<your-commit>", features = ["guest-std"] }
```

### Step 5: Rebuild

```bash
cargo clean
CC_riscv64imac_zero_linux_musl="$HOME/.zeroos/musl/bin/riscv64-linux-musl-gcc" \
CFLAGS_riscv64imac_zero_linux_musl="-mcmodel=medany -mabi=lp64 -march=rv64imac" \
cargo run --release -- \
  --tx-hash 0x8e303e5bb591a69307f292a605e8780156c41eff978c200c307cde12f043c2d4 \
  --network mainnet
```

## Key files in ZeroOS

| File | What |
|------|------|
| `crates/zeroos-os-linux/src/syscall.rs` | Syscall dispatch table (`sys_registry!` macro) |
| `crates/zeroos-os-linux/src/handlers/mod.rs` | `sys_noop()` and `sys_unsupported()` |
| `crates/zeroos-os-linux/src/handlers/vfs.rs` | File operation handlers |

## Other traps that may surface

After fixing ppoll, other unsupported syscalls may trigger similar crashes:

| Trap address | Syscall | Number |
|-------------|---------|--------|
| `0x800491b0` | `ppoll` | 73 |
| `0x80049748` | `rt_sigprocmask` | 135 |

If more traps appear, add more entries to `sys_registry!` with `sys_noop`.
