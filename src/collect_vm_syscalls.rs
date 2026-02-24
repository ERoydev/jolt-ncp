use std::sync::Arc;

use ckb_mock_tx_types::Resource;
use ckb_script::{ScriptGroup, TransactionScriptsVerifier, types::Machine};
use ckb_vm_syscall_tracer::{
    BinaryLocator, BinaryLocatorCollector, Collector, CollectorResult, SyscallBasedCollector,
    generated::traces::Syscalls,
};

use crate::collect_vm_creation::HumanReadableCycles;
use crate::vefier_context::{VerifierContext, VerifierGroupContext};

fn println_wrapper(message: &str) {
    // arch::println(message);
    println!("{}", message);
}

#[allow(dead_code)]
pub fn collect_vm_syscalls(
    verifier_context: &VerifierContext,
) -> CollectorResult<(BinaryLocator, Syscalls)> {
    let VerifierContext {
        verifier_resolve_transaction,
        verifier_resource,
        verifier_program_elf: _,
        verifier_consensus,
        verifier_tx_env,
        verifier_script_group,
        verifier_scheduler: _,
        verifier_sg_data: _,
    } = verifier_context;

    let collector: BinaryLocatorCollector<SyscallBasedCollector> =
        BinaryLocatorCollector::default();
    let collector_result: CollectorResult<(BinaryLocator, Syscalls)> = {
        let verifier: TransactionScriptsVerifier<Resource, _, Machine> =
            TransactionScriptsVerifier::new_with_generator(
                Arc::new(verifier_resolve_transaction.clone()),
                verifier_resource.clone(),
                verifier_consensus.clone(),
                verifier_tx_env.clone(),
                BinaryLocatorCollector::<SyscallBasedCollector>::syscall_generator,
                collector.clone(),
            );
        let collector_result = collector.collect(&verifier, verifier_script_group);
        match collector_result {
            Ok(data) => {
                println_wrapper(&format!("Run result: {}", data.exit_code));
                println_wrapper(&format!("All cycles: {}", HumanReadableCycles(data.cycles)));
                data
            }
            Err(err) => {
                println_wrapper(&format!("Run result: {}", err));
                std::process::exit(254);
            }
        }
    };
    collector_result
}

/// Run the syscall collector for one script group. Root VM for this script is always
/// [CollectorKey] `{ vm_id: 0, generation_id: 0 }` in the returned traces.
pub fn collect_vm_syscalls_for_group(
    base: &VerifierGroupContext,
    group: &ScriptGroup,
) -> CollectorResult<(BinaryLocator, Syscalls)> {
    let collector: BinaryLocatorCollector<SyscallBasedCollector> =
        BinaryLocatorCollector::default();
    let verifier: TransactionScriptsVerifier<Resource, _, Machine> =
        TransactionScriptsVerifier::new_with_generator(
            base.verifier_resolve_transaction.clone(), // Arc clone is cheap
            base.verifier_resource.clone(),
            base.verifier_consensus.clone(),
            base.verifier_tx_env.clone(),
            BinaryLocatorCollector::<SyscallBasedCollector>::syscall_generator,
            collector.clone(),
        );
    match collector.collect(&verifier, group) {
        Ok(data) => data,
        Err(err) => {
            eprintln!("collect_vm_syscalls_for_group: {}", err);
            std::process::exit(254);
        }
    }
}