# Musl Boot Syscalls vs ZeroOS

## Problem

Musl libc runs initialization code before `main()`. This init code uses Linux syscalls that ZeroOS doesn't support. Each unsupported syscall returns `-ENOSYS`, which musl treats as fatal → hits `sb zero, 0(zero)` crash trap → Jolt panics with `Illegal device store: Unknown memory mapping: 0x0`.

Fixing one syscall reveals the next. All must be stubbed.

## Syscalls needed for musl boot

| Syscall | Number | Why musl calls it | Stub behavior |
|---------|--------|-------------------|---------------|
| `SYS_ppoll` | 73 | Checks if fd 0/1/2 (stdin/stdout/stderr) are open during stdio init | Return `nfds` (all fds ready) — needs custom handler, not `sys_noop` |
| `SYS_openat` | 56 | Fallback: opens `/dev/null` if stdio fds aren't ready | Return 0 (fake success) |
| `SYS_close` | 57 | Closes file descriptors during cleanup | Return 0 |
| `SYS_fstat` | 80 | Gets file descriptor info during stdio setup | Return 0 |
| `SYS_ioctl` | 29 | Checks terminal capabilities on stdout/stderr | Return 0 |
| `SYS_mmap` | 222 | Allocates memory for internal buffers | Return 0 — **may need real handler if musl expects a valid pointer** |
| `SYS_rt_sigprocmask` | 135 | Sets signal mask during thread init | Return 0 (no signals in ZK guest) |
| `SYS_rt_sigaction` | 134 | Registers signal handlers | Return 0 (no signals in ZK guest) |

## Solution

In the ZeroOS fork, add all stubs to `sys_registry!` **outside any feature gate** (jolt doesn't enable `vfs`/`scheduler`/`memory` features):

```rust
sys_registry! {
    (SYS_exit, handlers::sys_exit, 1),
    (SYS_exit_group, handlers::sys_exit_group, 1),

    // Musl boot stubs
    (SYS_ppoll, handlers::sys_ppoll_stub, 5),
    (SYS_openat, handlers::sys_noop, 4),
    (SYS_close, handlers::sys_noop, 1),
    (SYS_fstat, handlers::sys_noop, 2),
    (SYS_ioctl, handlers::sys_noop, 3),
    (SYS_mmap, handlers::sys_noop, 6),
    (SYS_rt_sigprocmask, handlers::sys_noop, 4),
    (SYS_rt_sigaction, handlers::sys_noop, 4),

    // ... rest of feature-gated syscalls
}
```

`sys_ppoll_stub` returns `nfds` (second argument) so musl sees "all fds ready." Everything else uses `sys_noop` (returns 0).

## Syscall details and `sys_noop` safety

### `SYS_ppoll` (73) — I/O polling
Asks "are these file descriptors ready for reading/writing?" Musl uses it to check if stdin/stdout/stderr are open at boot.
- **`sys_noop` safe?** No. Returning 0 means "0 fds ready" — musl thinks fds are broken and tries fallback. Needs custom handler returning `nfds` (second arg).

### `SYS_openat` (56) — Open a file
Opens a file path, returns a file descriptor number. Musl calls it as fallback to open `/dev/null` when stdio check fails.
- **`sys_noop` safe?** Risky. Returns fd=0 (stdin), musl might overwrite stdin. But if ppoll returns correctly, musl should never reach this path.

### `SYS_close` (57) — Close a file descriptor
Releases a file descriptor.
- **`sys_noop` safe?** Yes. No-op close is harmless.

### `SYS_fstat` (80) — Get file info
Returns metadata (size, type, permissions) about a file descriptor. Musl uses it to check if stdout/stderr are terminals for line buffering.
- **`sys_noop` safe?** Risky. Returns "success" but leaves the output `struct stat` buffer uninitialized. Musl reads garbage. Probably harmless in ZK context since no real I/O happens.

### `SYS_ioctl` (29) — Device control
Sends control commands to devices, usually terminal queries like "what's your window size?" Musl checks if stdout is a terminal.
- **`sys_noop` safe?** Yes. Output buffer untouched but harmless in ZK context.

### `SYS_mmap` (222) — Map memory
Asks the OS for a chunk of memory. Returns a **pointer** to the allocated region. Musl uses it for internal buffers.
- **`sys_noop` safe?** **NO.** Returning 0 means "here's memory at address 0x0" — musl writes to 0x0 → same crash. Needs the real handler from ZeroOS's `memory` module or a stub that returns a valid address.

### `SYS_rt_sigprocmask` (135) — Set signal mask
Controls which signals are blocked for the current thread. Musl calls it during thread init.
- **`sys_noop` safe?** Yes. No signals exist in a ZK guest.

### `SYS_rt_sigaction` (134) — Register signal handler
Sets up a function to call when a signal arrives.
- **`sys_noop` safe?** Yes. No signals in ZK guest.

## Safety summary

| Syscall | `sys_noop` safe? | Risk |
|---------|-----------------|------|
| `ppoll` | **No** | Must return `nfds`, not 0 |
| `openat` | Risky | Returns fd=0, but shouldn't be reached if ppoll works |
| `close` | **Yes** | Harmless |
| `fstat` | Risky | Uninitialized buffer, probably fine for ZK |
| `ioctl` | **Yes** | Harmless |
| `mmap` | **NO** | Returns 0x0 as pointer → crash |
| `rt_sigprocmask` | **Yes** | Harmless |
| `rt_sigaction` | **Yes** | Harmless |

**`mmap` is the dangerous one.** Use ZeroOS's real `memory::sys_mmap` handler or enable the `memory` feature.
