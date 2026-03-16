---
status: ready
type: enhancement
created: 2026-03-15
---
# Quick: ckb-jolt-memory-shim

## Change Description
CKB-VM expects a zero-based 4MB flat memory layout, but Jolt maps guest DRAM at 0x8000_0000.
`FlatMemory` uses `byteorder::Cursor` and the memory module's `memset` (which uses `unsafe ptr::write_bytes`),
both of which can trigger alignment panics or illegal address faults in Jolt's constrained environment.

We implement a custom `JoltMemory` struct that fulfills the `ckb_vm::memory::Memory` trait using only
safe byte-level operations (`from_le_bytes`/`to_le_bytes` on slices), backed by a `Vec<u8>`.
The executor is updated to use `WXorXMemory<JoltMemory>` and to manually set the stack pointer (SP)
to prevent code overwrite.

## Affected Files
- `guest/src/jolt_memory.rs`: **NEW** — custom `Memory` implementation for Jolt
- `guest/src/executor.rs`: Replace `FlatMemory` with `JoltMemory`, set SP register
- `guest/src/lib.rs`: Add `mod jolt_memory`, uncomment VM execution in entrypoint

## Acceptance Criteria
### AC-1: JoltMemory implements Memory trait
Given a `JoltMemory` instance /
When CKB-VM interpreter calls load8/16/32/64 and store8/16/32/64 at addresses 0x0–0x3FFFFF /
Then all operations succeed using byte-level manipulation without `unsafe` or pointer casts

### AC-2: Address space shim
Given CKB-VM writes to address A near 0x0 /
When the write goes through JoltMemory /
Then it is redirected to index A in the internal `Vec<u8>` residing in Jolt's valid DRAM

### AC-3: Stack pointer initialization
Given the CKB-VM machine is configured with JoltMemory /
When the ELF is loaded /
Then SP (register 2) is manually set to 4194304 (end of 4MB buffer) before execution

### AC-4: Machine configuration
Given the executor creates a DefaultCoreMachine /
When using JoltMemory /
Then ISA is ISA_IMC | ISA_B | ISA_MOP, version is VERSION2, and memory wraps in WXorXMemory

## must_haves
truths: ["JoltMemory implements ckb_vm::memory::Memory<REG = u64>", "No unsafe blocks in jolt_memory.rs", "All load/store use from_le_bytes/to_le_bytes", "SP register set to RISCV_MAX_MEMORY before run"]
artifacts: ["guest/src/jolt_memory.rs"]
key_links: ["WXorXMemory<JoltMemory>", "set_register(SP, 4194304)", "from_le_bytes", "to_le_bytes"]

## Detected Conventions
- Rust edition 2021, workspace with guest crate
- CKB-VM v0.24.14, ckb_vm::Register trait for u64
- Guest uses `#[jolt::provable]` macro for entrypoint
- ExecOverride syscall handler pattern for intercepting spawned VMs
- Constants: RISCV_MAX_MEMORY=4MB, RISCV_PAGESIZE=4096, RISCV_PAGE_SHIFTS=12
