use std::u64;

use crate::exec_syscall_handler::EXEC_OVERRIDE;
use ckb_vm::{Bytes, CoreMachine, DefaultMachineBuilder, FlatMemory, SupportMachine};
use ckb_vm_fuzzing_utils::SynchronousSyscalls;
use protobuf_ckb_syscalls::ProtobufVmRunnerImpls;

const ELF_MAGIC: [u8; 4] = [0x7F, 0x45, 0x4C, 0x46];

pub struct VmExecutor {
    trace_data: Bytes,
    script_elf: Bytes,
    script_version: u8,
}

impl VmExecutor {
    pub fn new(trace_data: Bytes, script_elf: Bytes, script_version: u8) -> Self {
        Self {
            trace_data,
            script_elf,
            script_version,
        }
    }

    pub fn execute(&self) {
        if !self.script_elf.starts_with(&ELF_MAGIC) {
            panic!("Invalid ELF binary");
        }

        let machine_trace_impls = ProtobufVmRunnerImpls::new_with_bytes(&self.trace_data).unwrap();

        let machine_args: Vec<_> = machine_trace_impls
            .args()
            .iter()
            .map(|e| Ok(Bytes::copy_from_slice(e)))
            .collect();

        let machine_syscall = SynchronousSyscalls::new(machine_trace_impls);

        let machine_version = match self.script_version {
            0 => ckb_vm::machine::VERSION0,
            1 => ckb_vm::machine::VERSION1,
            2 => ckb_vm::machine::VERSION2,
            _ => panic!("Unsupported script version"),
        };

        // let machine_core =
        //     ckb_vm::DefaultCoreMachine::<u64, ckb_vm::WXorXMemory<FlatMemory<u64>>>::new(
        //         ckb_vm::ISA_IMC | ckb_vm::ISA_B | ckb_vm::ISA_MOP,
        //         machine_version,
        //         u64::MAX,
        //     );

        let machine_core = ckb_vm::DefaultCoreMachine::<u64, FlatMemory<u64>>::new(
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
