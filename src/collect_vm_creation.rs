// ! The file implements the collection of VMs created (and their ids) during the execution of a transaction.
// ! It includes methods from ckb-standalone-debugger to print the tree of VMs created.
// ! arch needs to be imported from ckb-standalone-debugger or replaced with STD println! but this would break OS-compatibility.

use ckb_vm_syscall_tracer::{
    BinaryLocator, BinaryLocatorCollector, Collector, CollectorKey, CollectorResult,
    VmCreateCollector,
    generated::traces::{VmCreation, VmCreations},
};

use crate::types::error::Result;
use std::{collections::HashMap, sync::Arc};

use ckb_script::{ScriptGroup, TransactionScriptsVerifier, types::Machine};

use ckb_mock_tx_types::Resource;

use crate::vefier_context::{VerifierContext, VerifierGroupContext};

fn println_wrapper(message: &str) {
    // arch::println(message);
    println!("{}", message);
}
pub struct HumanReadableCycles(pub u64);

impl std::fmt::Display for HumanReadableCycles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)?;
        if self.0 >= 1024 * 1024 {
            write!(f, "({:.1}M)", self.0 as f64 / 1024. / 1024.)?;
        } else if self.0 >= 1024 {
            write!(f, "({:.1}K)", self.0 as f64 / 1024.)?;
        }
        Ok(())
    }
}

fn collector_key_str(collector_key: &CollectorKey) -> String {
    if collector_key.generation_id != 0 {
        format!("{}/{}", collector_key.vm_id, collector_key.generation_id)
    } else {
        format!("{}", collector_key.vm_id)
    }
}

#[allow(dead_code)]
pub fn collect_vm_creation(
    verifier_context: &VerifierContext,
) -> CollectorResult<(BinaryLocator, VmCreations)> {
    // arch::println("Pre gather: collect vm creation");
    let VerifierContext {
        verifier_resolve_transaction,
        verifier_resource,
        verifier_consensus,
        verifier_tx_env,
        verifier_script_group,
        ..
    } = verifier_context;
    let collector: BinaryLocatorCollector<VmCreateCollector> = BinaryLocatorCollector::default();
    let verifier: TransactionScriptsVerifier<Resource, _, Machine> =
        TransactionScriptsVerifier::new_with_generator(
            Arc::new(verifier_resolve_transaction.clone()),
            verifier_resource.clone(),
            verifier_consensus.clone(),
            verifier_tx_env.clone(),
            BinaryLocatorCollector::<VmCreateCollector>::syscall_generator,
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
}

/// Count total VM instances (parent + all children) for a script group.
/// Returns the collector result and the total number of VM instances.
#[allow(dead_code)]
pub fn count_vm_instances_for_group(
    base: &VerifierGroupContext,
    group: &ScriptGroup,
) -> (CollectorResult<(BinaryLocator, VmCreations)>, usize) {
    let collector: BinaryLocatorCollector<VmCreateCollector> = BinaryLocatorCollector::default();
    let verifier: TransactionScriptsVerifier<Resource, _, Machine> =
        TransactionScriptsVerifier::new_with_generator(
            base.verifier_resolve_transaction.clone(), // Arc clone is cheap
            base.verifier_resource.clone(),
            base.verifier_consensus.clone(),
            base.verifier_tx_env.clone(),
            BinaryLocatorCollector::<VmCreateCollector>::syscall_generator,
            collector.clone(),
        );

    match collector.collect(&verifier, group) {
        Ok(data) => {
            let vm_count = data.traces.len(); // Total VM instances (parent + children)
            (data, vm_count)
        }
        Err(err) => {
            eprintln!("count_vm_instances_for_group: {}", err);
            std::process::exit(254);
        }
    }
}

fn print_vm_tree_recursive(
    tree: &HashMap<CollectorKey, Vec<VmCreation>>,
    hint: &HashMap<CollectorKey, String>,
    ckey: CollectorKey,
    prefix: &str,
    is_last: bool,
) -> Result<()> {
    let mut line = format!("Spawn tree: {}{}", prefix, collector_key_str(&ckey));
    if line.chars().count() < 32 {
        line.push_str(String::from(" ").repeat(32 - line.chars().count()).as_str());
    }
    line.push(' ');
    line.push_str(hint.get(&ckey).unwrap());
    println!("{}", line);

    // Get children, if any
    if let Some(children) = tree.get(&ckey) {
        // Update prefix for children
        let new_prefix = if prefix.is_empty() {
            "".to_string()
        } else if is_last {
            format!("{}    ", prefix.trim_end_matches("├── ").trim_end_matches("└── "))
        } else {
            format!("{}│   ", prefix.trim_end_matches("├── ").trim_end_matches("└── "))
        };
        // Print each child
        for (i, child) in children.iter().enumerate() {
            let is_last_child = i == children.len() - 1;
            let child_prefix = if is_last_child {
                format!("{}└── ", new_prefix)
            } else {
                format!("{}├── ", new_prefix)
            };
            print_vm_tree_recursive(
                tree,
                hint,
                CollectorKey { vm_id: child.vm_id, generation_id: child.generation_id },
                &child_prefix,
                is_last_child,
            )?;
        }
    }
    Ok(())
}

/// Print the VM spawn tree showing parent-child relationships.
/// Useful for debugging to verify both parent and child VMs are executed.
#[allow(dead_code)]
pub fn print_vm_tree(
    collector_result: &CollectorResult<(BinaryLocator, VmCreations)>,
) -> Result<()> {
    let mut hint = HashMap::new();
    let mut tree = HashMap::new();
    collector_result.traces.iter().for_each(|(k, v)| {
        hint.insert(
            k.clone(),
            format!(
                "{}[{}][{}..{}]",
                match v.0.source {
                    0x1 => "input",
                    0x2 => "output",
                    0x3 => "cell_dep",
                    0x4 => "header_dep",
                    0x0100000000000001 => "group_input",
                    0x0100000000000002 => "group_output",
                    _ => unreachable!(),
                },
                v.0.index,
                v.0.offset,
                v.0.offset + v.0.length
            ),
        );
        if !v.1.vm_creations.is_empty() {
            tree.insert(k.clone(), v.1.vm_creations.clone());
        }
    });
    if !tree.is_empty() {
        print_vm_tree_recursive(
            &tree,
            &hint,
            CollectorKey { vm_id: 0, generation_id: 0 },
            "",
            false,
        )?;
    }
    Ok(())
}