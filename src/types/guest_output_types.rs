// GUEST OUTPUT TYPES DESERIALIZATION

use serde::{Deserialize, Serialize};


// Define GuestOutput locally for deserialization (matches guest's types.rs)
#[derive(Debug, Serialize, Deserialize)]
pub struct GuestOutput {
    pub transaction_hash: [u8; 32],
}

// Using currently for vm_replay_results verification from guest output
#[derive(Debug, Serialize, Deserialize)]
pub struct VmExecutionResult {
    pub vm_index: u32,
    pub exit_code: i8,
    pub cycles: u64,
}

// Using currently for trace details verification from guest output
#[derive(Debug, Serialize, Deserialize)]
pub struct TraceDetail {
    pub index: u32,
    pub script_version: u8,
    pub script_group_type: u8,

    pub script_hash: [u8; 32],
    pub trace_data_len: u32,
    pub program_elf_len: u32,
}
