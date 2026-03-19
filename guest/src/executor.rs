use std::u64;

use crate::exec_syscall_handler::EXEC_OVERRIDE;
use ckb_vm::{Bytes, CoreMachine, DefaultMachineBuilder, FlatMemory, SupportMachine};
use ckb_vm_fuzzing_utils::SynchronousSyscalls;
use protobuf_ckb_syscalls::ProtobufVmRunnerImpls;

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

        // let machine_core = ckb_vm::DefaultCoreMachine::<u64, ckb_vm::WXorXMemory<JoltMemory>>::new(
        //     ckb_vm::ISA_IMC | ckb_vm::ISA_B | ckb_vm::ISA_MOP,
        //     machine_version,
        //     VM_MAX_CYCLES,
        // );

        let machine_core =
            ckb_vm::DefaultCoreMachine::<u64, ckb_vm::WXorXMemory<FlatMemory<u64>>>::new(
                ckb_vm::ISA_IMC | ckb_vm::ISA_B | ckb_vm::ISA_MOP,
                machine_version,
                u64::MAX,
            );

        let mut machine = DefaultMachineBuilder::new(machine_core)
            .syscall(Box::new(EXEC_OVERRIDE))
            .syscall(Box::new(machine_syscall))
            .build();

        machine
            .load_program(&self.script_elf, machine_args.into_iter())
            .unwrap();

        machine.run().expect("CKB-VM execution failed");
    }
}
