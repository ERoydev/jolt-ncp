use std::collections::HashMap;

use crate::types::ncp_types;
use ckb_script::{ScriptGroupType, ScriptVersion};
use ckb_types::{
    bytes::Bytes,
    packed::{Byte32, Header, Transaction},
    prelude::*,
};

#[derive(Clone, Debug)]
pub struct HistoricalWitness {
    pub header: Header,
    pub full_transactions: Vec<Transaction>,
}
/// One transaction has many scripts
/// For efficiency, we can group the scripts by (script_version, script_group_type, script_hash)
/// and collect in one collector the syscall traces for each script group
#[derive(Clone, Debug)]
pub struct TransactionProofContext {
    // NOTE: perhaps we should use MockTransaction instead?
    pub transaction: Vec<u8>,
    pub vm_traces: Vec<ScriptGroupTraces>,
}

impl TransactionProofContext {
    /// Serialize to raw bytes for sending to SP1 guest using serde/bincode
    pub fn to_raw_bytes(&self) -> crate::types::error::Result<Vec<u8>> {
        let raw_input: ncp_types::TransactionProofContext =
            ncp_types::TransactionProofContext::from(self);

        bincode::serialize(&raw_input).map_err(|e| {
            crate::types::error::HostError::TransactionProofContextConversion(e.to_string())
        })
    }
}

impl From<&TransactionProofContext> for ncp_types::TransactionProofContext {
    fn from(ctx: &TransactionProofContext) -> Self {
        let mut code_hash_to_index: HashMap<Byte32, u16> = HashMap::new();
        let mut machine_program_elfs: Vec<Bytes> = Vec::new();

        for trace in ctx.vm_traces.iter() {
            let elf_hash = trace.code_hash.clone();
            code_hash_to_index.entry(elf_hash).or_insert_with(|| {
                let idx = machine_program_elfs.len() as u16;
                machine_program_elfs.push(trace.machine_program_elf.clone());
                idx
            });
        }

        let vm_traces: Vec<ncp_types::ScriptGroupTraces> = ctx
            .vm_traces
            .iter()
            .map(|t| {
                let elf_index = code_hash_to_index
                    .get(&t.code_hash)
                    .expect("Elf index should be obtained");

                let version_u8 = match t.script_version {
                    ScriptVersion::V0 => 0u8,
                    ScriptVersion::V1 => 1u8,
                    ScriptVersion::V2 => 2u8,
                };
                let group_type_u8 = match t.script_group_type {
                    ScriptGroupType::Lock => 0u8,
                    ScriptGroupType::Type => 1u8,
                };
                let mut hash_arr = [0u8; 32];
                hash_arr.copy_from_slice(t.script_hash.as_slice());

                ncp_types::ScriptGroupTraces {
                    script_version: version_u8,
                    script_group_type: group_type_u8,
                    script_hash: hash_arr,
                    machine_trace_data: t.machine_trace_data.to_vec(),
                    machine_program_elf_index: *elf_index,
                }
            })
            .collect();

        ncp_types::TransactionProofContext {
            raw_transaction_bytes: ctx.transaction.clone(),
            vm_traces,
            machine_program_elfs: machine_program_elfs
                .into_iter()
                .map(|b| b.to_vec())
                .collect(),
        }
    }
}

impl From<&TransactionProofContext> for guest::TransactionProofContext {
    fn from(ctx: &TransactionProofContext) -> Self {
        let mut code_hash_to_index: HashMap<Byte32, u16> = HashMap::new();
        let mut machine_program_elfs: Vec<Bytes> = Vec::new();

        for trace in ctx.vm_traces.iter() {
            let elf_hash = trace.code_hash.clone();
            code_hash_to_index.entry(elf_hash).or_insert_with(|| {
                let idx = machine_program_elfs.len() as u16;
                machine_program_elfs.push(trace.machine_program_elf.clone());
                idx
            });
        }

        let vm_traces: Vec<guest::ScriptGroupTraces> = ctx
            .vm_traces
            .iter()
            .map(|t| {
                let elf_index = code_hash_to_index
                    .get(&t.code_hash)
                    .expect("Elf index should be obtained");

                let version_u8 = match t.script_version {
                    ScriptVersion::V0 => 0u8,
                    ScriptVersion::V1 => 1u8,
                    ScriptVersion::V2 => 2u8,
                };
                let group_type_u8 = match t.script_group_type {
                    ScriptGroupType::Lock => 0u8,
                    ScriptGroupType::Type => 1u8,
                };
                let mut hash_arr = [0u8; 32];
                hash_arr.copy_from_slice(t.script_hash.as_slice());

                guest::ScriptGroupTraces {
                    script_version: version_u8,
                    script_group_type: group_type_u8,
                    script_hash: hash_arr,
                    machine_trace_data: t.machine_trace_data.to_vec(),
                    machine_program_elf_index: *elf_index,
                }
            })
            .collect();

        guest::TransactionProofContext {
            raw_transaction_bytes: ctx.transaction.clone(),
            vm_traces,
            machine_program_elfs: machine_program_elfs
                .into_iter()
                .map(|b| b.to_vec())
                .collect(),
        }
    }
}

/// A collection of all traces for a script group
/// Where a script group is (script_version, script_group_type, script_hash)
#[derive(Clone, Debug)]
pub struct ScriptGroupTraces {
    pub script_version: ScriptVersion,
    pub script_group_type: ScriptGroupType,
    pub script_hash: Byte32,
    pub code_hash: Byte32,
    pub machine_trace_data: Bytes,
    pub machine_program_elf: Bytes,
}
