use ckb_vm::{Register, SupportMachine, Syscalls};
use ckb_vm_fuzzing_utils::SyscallCode;

#[derive(Clone, Copy)]
pub struct ExecOverride;

pub static EXEC_OVERRIDE: ExecOverride = ExecOverride;

impl<Mac: SupportMachine> Syscalls<Mac> for ExecOverride {
    fn initialize(&mut self, _machine: &mut Mac) -> Result<(), ckb_vm::Error> {
        Ok(())
    }

    fn ecall(&mut self, machine: &mut Mac) -> Result<bool, ckb_vm::Error> {
        let Ok(syscall_code): Result<SyscallCode, _> =
            machine.registers()[ckb_vm::registers::A7].to_u64().try_into()
        else {
            return Ok(false);
        };

        if syscall_code == SyscallCode::Exec {
            machine.set_register(ckb_vm::registers::A0, Mac::REG::from_u64(0));
            return Ok(true);
        }

        Ok(false) 
    }
}