//! Guest types for deserialization
//!
//! These types are self-contained and don't depend on any external CKB crates.
//! They mirror the structure expected from the host but use only primitive types
//! and standard library types.

use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
pub struct TransactionProofContext {
    pub raw_transaction_bytes: Vec<u8>,
    pub vm_traces: Vec<ScriptGroupTraces>,
    pub machine_program_elfs: Vec<Vec<u8>>,
}

/// Traces for a single script group
#[derive(Serialize, Deserialize)]

pub struct ScriptGroupTraces {
    pub script_version: u8,
    pub script_group_type: u8,
    pub script_hash: [u8; 32],
    pub machine_trace_data: Vec<u8>,
    pub machine_program_elf_index: u16,
}

/// Output committed by the SP1 guest
#[derive(Serialize, Deserialize)]
pub struct GuestOutput {
    pub transaction_hash: [u8; 32],
}