use jolt::{end_cycle_tracking, start_cycle_tracking};
use serde::{Deserialize, Serialize};
mod exec_syscall_handler;
mod executor;
mod jolt_memory;

/// Transaction proof context sent from host
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TransactionProofContext {
    pub raw_transaction_bytes: Vec<u8>,
    pub vm_traces: Vec<ScriptGroupTraces>,
    pub machine_program_elfs: Vec<Vec<u8>>,
}

/// Traces for a single script group
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ScriptGroupTraces {
    pub script_version: u8,
    pub script_group_type: u8,
    pub script_hash: [u8; 32],
    pub machine_trace_data: Vec<u8>,
    pub machine_program_elf_index: u16,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GuestOutput {
    pub transaction_hash: [u8; 32],
}
// TODO: Agjust `max_trace_lenght` or find a way to make it exactly how much we need.
// The reason is that the trace length is determined by the prover's padding strategy for witness polynomials, which currently pads to the next power of 2.
// So if we set it to 67M, the prover will pad to 67M instead of 134M, which saves a lot of unnecessary padding and commitment time.

/*
Defaults:
    DEFAULT_HEAP_SIZE: 128MB
    DEFAULT_STACK_SIZE: 4KB
    DEFAULT_MAX_OUTPUT_SIZE: 4KB
    DEFAULT_MAX_INPUT_SIZE: 4KB
    DEFAULT_MAX_TRACE_LENGTH: 2^^24
    RAM_START_ADDRESS: 0x80000000

Parameters i have set:
    - max_input_size = 2_000_000 -> 1.9 MB, since the transaction proof context can be large due to the traces and ELF binaries
    - max_trace_length = 67_108_864 - > 67M cycles, thats 4x the default.
        This is the most expensive parameter - all witness polynomials are padded to this size,
        so the prover need to commit over 67M entries regardless of actual execution length.
    - stack_size = 65536 -> Default is 4096 bytes, i set 64KB
*/

#[jolt::provable(
    max_input_size = 1_200_000,
    max_trace_length = 1_073_741_824, // 2^20
    stack_size = 65536
)]
fn entrypoint(tx_context: TransactionProofContext) -> GuestOutput {
    // tx_context is automatically deserialized by Jolt using postcard
    start_cycle_tracking("ckb-vm replay");

    // tx_context.vm_traces.iter().for_each(|trace| {
    //     executor::VmExecutor::new(
    //         &trace.machine_trace_data,
    //         &tx_context.machine_program_elfs[trace.machine_program_elf_index as usize],
    //         trace.script_version,
    //     )
    //     .execute()
    // });

    let trace = &tx_context.vm_traces[0];
    executor::VmExecutor::new(
        &trace.machine_trace_data,
        &tx_context.machine_program_elfs[trace.machine_program_elf_index as usize],
        trace.script_version,
    )
    .execute();

    // TODO: replace with ckb_hash::blake2b_256 once the crate is added to guest deps
    let transaction_hash = [0u8; 32];
    // let tx_hash = ckb_hash::blake2b_256(&tx_context.raw_transaction_bytes);
    end_cycle_tracking("ckb-vm replay");
    GuestOutput {
        transaction_hash: transaction_hash,
    }
}
