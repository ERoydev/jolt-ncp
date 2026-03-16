
1. 
Jolt custom RISC-V target spec uses "64" (string) for `target-pointer-width`, but with Rust 1.92 it expects a number.
Jolt requires Rust 1.88, Rust 1.92 broke compatibility with Jolt's custom target spec

ckb-vm-0.24.14 - requires rust 1.92

2. Input size
- The input i try to give is like 1.1MB, but Jolt's default `max_input_size` is only 4096 bytes (4KB)

3. `Illegal device store: Unknown memory mapping: 0x0` panic error:
- The nested VM. `ckb-vm` initializes its virtual memory starting at address `0x0`, but Jolt's memory layout doesn't include `0x0` as a valid region.

Findings:
- ckb-vm treats all addresses as 0-based virtual addresses
- Memory trait implementations of ckb-vm directly use addresses from 0 to `memory_size`
- no built-in base address offset mechanism

Final:
- ckb-vm inside jolt zkVM guests fails because of incompatible memory models.

I have to modify the ckb-vm (rewrite the ELF loader, machine core, all memory access)
Or Jolt adding support for custom memory mappings 

### Summary
The Problem:
  - Jolt's RAM starts at 0x80000000 - all memory accesses must be at this address or higher
  - ckb-vm's memory starts at 0x0 - it uses addresses 0 to ~4MB
  - When ckb-vm writes to address 0x0, Jolt panics because that address doesn't exist in its memory map

Modifying ckb-vm:
- i must offset the ELF loading 
- offset stack pointer - vm initiallizes sp near the top of its 4MB space.
- offset syscall return addresses - when syscalls write data to memory, they use guest addresses.
- the elf internal references - if code has `lui a0, 0x10` thats baked into binary, i cant just offset at runtime

The real problem: CKB script binaries are compiled to run at address 0. You can't just add an offset wrapper - the binaries themselves contain hardcoded 0-based addresse