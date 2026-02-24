use serde::{Deserialize, Serialize};

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

#[jolt::provable(max_input_size = 2097152, stack_size = 1048576, heap_size = 268435456, max_trace_length = 67108864)]
fn entrypoint(tx_context: TransactionProofContext) {
    // tx_context is automatically deserialized by Jolt using postcard
    let _tx_len = tx_context.raw_transaction_bytes.len();
    let _traces_count = tx_context.vm_traces.len();
    let _elfs_count = tx_context.machine_program_elfs.len();
}
