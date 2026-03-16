use jolt::{end_cycle_tracking, println, start_cycle_tracking};
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

#[jolt::provable(max_input_size = 2_000_000, max_trace_length = 67_108_864)]
fn entrypoint(tx_context: TransactionProofContext) -> GuestOutput {
    // tx_context is automatically deserialized by Jolt using postcard
    start_cycle_tracking("ckb-vm replay");

    tx_context.vm_traces.iter().for_each(|trace| {
        executor::VmExecutor::new(
            &trace.machine_trace_data,
            &tx_context.machine_program_elfs[trace.machine_program_elf_index as usize],
            trace.script_version,
        )
        .execute()
    });

    // TODO: replace with ckb_hash::blake2b_256 once the crate is added to guest deps
    // let transaction_hash = [0u8; 32];
    let tx_hash = ckb_hash::blake2b_256(&tx_context.raw_transaction_bytes);
    end_cycle_tracking("ckb-vm replay");
    GuestOutput {
        transaction_hash: tx_hash,
    }
}
