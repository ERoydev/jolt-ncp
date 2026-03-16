mod cli;
mod collect_vm_creation;
mod collect_vm_syscalls;
mod config;
mod macros;
mod rpc_client;
mod tx_converter;
mod type_id;
mod types;
mod vefier_context;

use crate::types::error::{HostError, Result};
use ckb_script::{ScriptGroup, ScriptGroupType, ScriptVersion};
use ckb_types::bytes::Bytes;
use ckb_types::packed::{Byte32, RawTransaction};
use ckb_types::prelude::*;
use ckb_vm_syscall_tracer::generated::traces::Syscalls;
use ckb_vm_syscall_tracer::{BinaryLocator, CollectorKey, CollectorResult};
use jolt_sdk::host::Program;
use types::types::TransactionProofContext;

use crate::collect_vm_syscalls::collect_vm_syscalls_for_group;
use crate::rpc_client::{RpcClient, DEFAULT_RPC_TIMEOUT_SECONDS};
use crate::tx_converter::tx_to_mock_tx;
use crate::type_id::is_type_id_group;
use crate::types::types::ScriptGroupTraces;
use crate::vefier_context::VerifierGroupContext;

pub use crate::cli::HostArgs;
pub use crate::config::HostConfig;

pub struct HostExecutor {
    rpc_client: RpcClient,
    network: String,
}

impl HostExecutor {
    pub fn new(config: HostConfig) -> Result<Self> {
        let rpc_client = RpcClient::new(config.rpc_url.as_str(), DEFAULT_RPC_TIMEOUT_SECONDS)
            .map_err(|e| HostError::RpcClientCreation(e.to_string()))?;

        Ok(Self {
            rpc_client,
            network: config.network,
        })
    }

    pub fn run(&self, tx_hash: &str, program: Program) -> Result<()> {
        let tx_hash = if tx_hash.starts_with("0x") {
            tx_hash.to_string()
        } else {
            format!("0x{}", tx_hash)
        };

        let transaction = self
            .rpc_client
            .get_transaction(&tx_hash)
            .exec(&self.rpc_client)
            .map_err(|e| HostError::TransactionFetch(e.to_string()))?;

        // Cellbase transactions (first tx in a block) have no scripts to verify.
        // They don't have real input cells - just a null OutPoint placeholder, and point to 0x0...0 transactions
        // Output scripts are verified when those cells are spent, not when created.
        if transaction.is_cellbase() {
            println!("Transaction is a cellbase (miner reward) - no scripts to verify");
            println!(
                "Cellbase transactions are validated by block-level rules, not script execution."
            );
            return Ok(());
        }

        let mock_transaction = tx_to_mock_tx(&self.rpc_client, &transaction).unwrap();
        let raw_transaction = transaction.raw();

        // Inject Type ID ELF if the transaction uses Type ID scripts.
        // Type ID is a built-in CKB script with no on-chain binary, so we provide one.
        // We will also need to override the code_hash of the Type ID script
        // because we will never be able to match the magic constant TYPE_ID_CODE_HASH
        // inject_type_id_if_needed(&mut mock_transaction);

        let (verifier_base, groups) = VerifierGroupContext::from_mock_transaction_with_groups(
            &mock_transaction,
            ScriptVersion::V2,
        )
        .unwrap();

        let script_traces = self.accumulate_traces(&verifier_base, &groups)?;

        // Call the Jolt guest with the collected traces
        self.execute(&raw_transaction, script_traces, program);
        Ok(())
    }

    fn execute(
        &self,
        raw_tx: &RawTransaction,
        script_traces: Vec<ScriptGroupTraces>,
        mut program: Program,
    ) {
        let raw_encoded_tx: Vec<u8> = raw_tx.as_slice().to_vec();

        // Build TransactionProofContext (host type)
        let proof_context = TransactionProofContext {
            transaction: raw_encoded_tx,
            vm_traces: script_traces,
        };

        // Convert to guest type (Jolt handles serialization automatically)
        let guest_context: guest::TransactionProofContext =
            guest::TransactionProofContext::from(&proof_context);

        // Preprocessing
        let shared_preprocessing = guest::preprocess_shared_entrypoint(&mut program);
        let prover_preprocessing =
            guest::preprocess_prover_entrypoint(shared_preprocessing.clone());
        let verifier_setup = prover_preprocessing.generators.to_verifier_setup();
        let verifier_preprocessing =
            guest::preprocess_verifier_entrypoint(shared_preprocessing, verifier_setup);

        // Build prover and verifier
        let prove = guest::build_prover_entrypoint(program, prover_preprocessing);
        let verify = guest::build_verifier_entrypoint(verifier_preprocessing);

        // Prove execution
        println!("Starting proof generation...");
        let (output, proof, io_device) = prove(guest_context.clone());
        println!("Proof generated!");
        println!("Output: {:?}", output);

        // Verify the proof
        // let is_valid = verify(guest_context, output, io_device.panic, proof);
        // println!("Proof valid: {}", is_valid);
    }

    fn accumulate_traces(
        &self,
        verifier_base: &VerifierGroupContext,
        groups: &[(ScriptGroupType, Byte32, ScriptGroup, ScriptVersion)],
    ) -> Result<Vec<ScriptGroupTraces>> {
        let mut script_traces: Vec<ScriptGroupTraces> = Vec::with_capacity(groups.len());
        let mut collector_results: Vec<CollectorResult<(BinaryLocator, Syscalls)>> =
            Vec::with_capacity(groups.len());

        for (script_group_type, script_hash, group, script_version) in groups {
            // Skip Type ID scripts and perform a simple constraint check
            // TODO: we will need to perform a simple constraint check here
            if is_type_id_group(group) {
                println!(
                    "Skipping Type ID script {:?} hash={:x} (built-in, no binary)",
                    script_group_type, script_hash
                );
                continue;
            }

            // collect syscalls for all VMs (root + children)
            let syscalls_collector_result: CollectorResult<(BinaryLocator, Syscalls)> =
                collect_vm_syscalls_for_group(verifier_base, group);

            // the VM count (including child process VMs)
            let vm_count = syscalls_collector_result.traces.len();

            println!(
                "Script {:?} hash={:x} - Total VM instances: {}",
                script_group_type, script_hash, vm_count
            );

            if vm_count > 1 {
                println!("  -> Child VMs detected (spawn was called)");
                for key in syscalls_collector_result.traces.keys() {
                    println!("    VM id={}, generation={}", key.vm_id, key.generation_id);
                }
                // Run VmCreateCollector to get the spawn tree structure
                // let (vm_creation_result, _) = count_vm_instances_for_group(verifier_base, group);
                // print_vm_tree(&vm_creation_result);
            }

            // an ugly but efficient O(1) ELF lookup using pre-built map
            let machine_program_elf = verifier_base
                .get_program_elf(&group.script)
                .map_err(|_| HostError::ProgramElfNotFound)?;

            let machine_collector_key = CollectorKey {
                vm_id: 0,
                generation_id: 0,
            };
            let machine_trace_data =
                match syscalls_collector_result.traces.get(&machine_collector_key) {
                    Some(data) => {
                        let vec: Vec<u8> = data.1.clone().into();
                        Bytes::from(vec)
                    }
                    None => Bytes::new(),
                };
            script_traces.push(ScriptGroupTraces {
                script_version: *script_version,
                script_group_type: *script_group_type,
                script_hash: script_hash.clone(),
                code_hash: group.script.code_hash(),
                machine_trace_data,
                machine_program_elf,
            });
            collector_results.push(syscalls_collector_result);
        }

        for (i, (st, result)) in script_traces
            .iter()
            .zip(collector_results.iter())
            .enumerate()
        {
            let root_key = CollectorKey {
                vm_id: 0,
                generation_id: 0,
            };
            let root_trace = result.traces.get(&root_key);
            println!(
                "script {}: version={:?} type={:?} hash={:x} exit_code={} ckb_cycles={} root_trace_present={} total_vms={}",
                i,
                st.script_version,
                st.script_group_type,
                st.script_hash,
                result.exit_code,
                result.cycles,
                root_trace.is_some(),
                result.traces.len(),
            );
        }

        Ok(script_traces)
    }
}
