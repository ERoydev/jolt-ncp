use crate::exec_syscall_handler::EXEC_OVERRIDE;
use crate::jolt_memory::JoltMemory;
use ckb_vm::{Bytes, CoreMachine, DefaultMachineBuilder, SupportMachine};
use ckb_vm_fuzzing_utils::SynchronousSyscalls;
use protobuf_ckb_syscalls::ProtobufVmRunnerImpls;

pub const VM_MAX_CYCLES: u64 = u64::MAX;

pub struct VmExecutor {
    trace_data: Bytes,
    script_elf: Bytes,
    script_version: u8,
}

impl VmExecutor {
    pub fn new(trace_data: &[u8], script_elf: &[u8], script_version: u8) -> Self {
        Self {
            trace_data: Bytes::copy_from_slice(trace_data),
            script_elf: Bytes::copy_from_slice(script_elf),
            script_version,
        }
    }

    pub fn execute(&self) {
        let machine_trace_impls = ProtobufVmRunnerImpls::new_with_bytes(&self.trace_data).unwrap();

        let args_data = machine_trace_impls.args().to_vec();
        let machine_args = args_data.iter().map(|e| Ok(Bytes::copy_from_slice(e)));

        let machine_syscall = SynchronousSyscalls::new(machine_trace_impls);

        let machine_version = match self.script_version {
            0 => ckb_vm::machine::VERSION0,
            1 => ckb_vm::machine::VERSION1,
            2 => ckb_vm::machine::VERSION2,
            _ => panic!("Unsupported script version"),
        };

        // Use JoltMemory (safe byte-level shim) instead of FlatMemory.
        // JoltMemory redirects CKB-VM's 0-based addresses into a Vec<u8>
        // that resides in Jolt's valid DRAM region.
        let machine_core = ckb_vm::DefaultCoreMachine::<u64, ckb_vm::WXorXMemory<JoltMemory>>::new(
            ckb_vm::ISA_IMC | ckb_vm::ISA_B | ckb_vm::ISA_MOP,
            machine_version,
            VM_MAX_CYCLES,
        );

        let machine_builder = DefaultMachineBuilder::new(machine_core)
            .syscall(Box::new(EXEC_OVERRIDE))
            .syscall(Box::new(machine_syscall));

        let mut machine = machine_builder.build();

        machine
            .load_program(&self.script_elf, machine_args.into_iter())
            .unwrap();

        // Force SP to the top of the 4 MB buffer so the stack grows
        // downward into free space and cannot overwrite code loaded at
        // the low end of memory.
        machine.set_register(ckb_vm::registers::SP, ckb_vm::RISCV_MAX_MEMORY as u64);

        machine.run().expect("CKB-VM execution failed");
    }
}
