//! The exception module contains all the exception kinds and the function to handle exceptions.

use crate::{
    cpu::{Cpu, Mode},
    csr::*,
};

/// All the exception kinds.
#[derive(Debug)]
pub enum Exception {
    InstructionAddressMisaligned,
    InstructionAccessFault,
    IllegalInstruction(String),
    Breakpoint,
    LoadAddressMisaligned,
    LoadAccessFault,
    StoreAMOAddressMisaligned,
    StoreAMOAccessFault,
    EnvironmentCallFromUMode,
    EnvironmentCallFromSMode,
    EnvironmentCallFromMMode,
    InstructionPageFault,
    LoadPageFault,
    StoreAMOPageFault,
}

impl Exception {
    fn exception_code(&self) -> i64 {
        match self {
            Exception::InstructionAddressMisaligned => 0,
            Exception::InstructionAccessFault => 1,
            Exception::IllegalInstruction(_s) => 2,
            Exception::Breakpoint => 3,
            Exception::LoadAddressMisaligned => 4,
            Exception::LoadAccessFault => 5,
            Exception::StoreAMOAddressMisaligned => 6,
            Exception::StoreAMOAccessFault => 7,
            Exception::EnvironmentCallFromUMode => 8,
            Exception::EnvironmentCallFromSMode => 9,
            Exception::EnvironmentCallFromMMode => 11,
            Exception::InstructionPageFault => 12,
            Exception::LoadPageFault => 13,
            Exception::StoreAMOPageFault => 15,
        }
    }
    /// Update CSRs and the program counter depending on an exception.
    pub fn take_trap(&self, cpu: &mut Cpu) -> Result<(), Exception> {
        let exception_pc = (cpu.pc as i64) - 4;

        let medeleg = cpu.state.read(MEDELEG);
        let sedeleg = cpu.state.read(SEDELEG);
        let pos = self.exception_code() & 0xffff;
        match ((medeleg >> pos) & 1) == 0 {
            true => cpu.mode = Mode::Machine,
            false => match ((sedeleg >> pos) & 1) == 0 {
                true => cpu.mode = Mode::Supervisor,
                false => cpu.mode = Mode::User,
            },
        }

        dbg!(format!("EXCEPTION {:?} next mode {:#?}", self, cpu.mode));

        match cpu.mode {
            Mode::Machine => {
                // Set the program counter to the machine trap-handler base address (mtvec).
                cpu.pc = (cpu.state.read(MTVEC) & !1) as usize;

                // 3.1.15 Machine Exception Program Counter (mepc)
                // "The low bit of mepc (mepc[0]) is always zero."
                // "When a trap is taken into M-mode, mepc is written with the virtual address of
                // the instruction that was interrupted or that encountered the exception.
                // Otherwise, mepc is never written by the implementation, though it may be
                // explicitly written by software."
                cpu.state.write(MEPC, exception_pc & !1);

                // 3.1.16 Machine Cause Register (mcause)
                // "When a trap is taken into M-mode, mcause is written with a code indicating
                // the event that caused the trap. Otherwise, mcause is never written by the
                // implementation, though it may be explicitly written by software."
                cpu.state.write(MCAUSE, 1 << 63 | self.exception_code());

                // 3.1.17 Machine Trap Value (mtval) Register
                // "When a trap is taken into M-mode, mtval is either set to zero or written with
                // exception-specific information to assist software in handling the trap.
                // Otherwise, mtval is never written by the implementation, though it may be
                // explicitly written by software."
                // "When a hardware breakpoint is triggered, or an instruction-fetch, load, or
                // store address-misaligned, access, or page-fault exception occurs, mtval is
                // written with the faulting virtual address. On an illegal instruction trap,
                // mtval may be written with the first XLEN or ILEN bits of the faulting
                // instruction as described below. For other traps, mtval is set to zero."
                cpu.state.write(MTVAL, exception_pc);

                // Set a privious interrupt-enable bit for supervisor mode (MPIE, 7) to the value
                // of a global interrupt-enable bit for supervisor mode (MIE, 3).
                cpu.state
                    .write_bit(MSTATUS, 7, cpu.state.read_bit(MSTATUS, 3));
                // Set a global interrupt-enable bit for supervisor mode (MIE, 3) to 0.
                cpu.state.write_bit(MSTATUS, 3, false);
                // Set a privious privilege mode for supervisor mode (MPP, 11..13) to 0.
                cpu.state.write_bits(MSTATUS, 11..13, 0b00);
            }
            Mode::Supervisor => {
                // Set the program counter to the supervisor trap-handler base address (stvec).
                cpu.pc = (cpu.state.read(STVEC) & !1) as usize;

                // 4.1.9 Supervisor Exception Program Counter (sepc)
                // "The low bit of sepc (sepc[0]) is always zero."
                // "When a trap is taken into S-mode, sepc is written with the virtual address of
                // the instruction that was interrupted or that encountered the exception.
                // Otherwise, sepc is never written by the implementation, though it may be
                // explicitly written by software."
                cpu.state.write(SEPC, exception_pc & !1);

                // 4.1.10 Supervisor Cause Register (scause)
                // "When a trap is taken into S-mode, scause is written with a code indicating
                // the event that caused the trap.  Otherwise, scause is never written by the
                // implementation, though it may be explicitly written by software."
                cpu.state.write(SCAUSE, 1 << 63 | self.exception_code());

                // 4.1.11 Supervisor Trap Value (stval) Register
                // "When a trap is taken into S-mode, stval is written with exception-specific
                // information to assist software in handling the trap. Otherwise, stval is never
                // written by the implementation, though it may be explicitly written by software."
                // "When a hardware breakpoint is triggered, or an instruction-fetch, load, or
                // store address-misaligned, access, or page-fault exception occurs, stval is
                // written with the faulting virtual address. On an illegal instruction trap,
                // stval may be written with the first XLEN or ILEN bits of the faulting
                // instruction as described below. For other exceptions, stval is set to zero."
                cpu.state.write(STVAL, exception_pc);

                // Set a privious interrupt-enable bit for supervisor mode (SPIE, 5) to the value
                // of a global interrupt-enable bit for supervisor mode (SIE, 1).
                cpu.state
                    .write_bit(SSTATUS, 5, cpu.state.read_bit(SSTATUS, 1));
                // Set a global interrupt-enable bit for supervisor mode (SIE, 1) to 0.
                cpu.state.write_bit(SSTATUS, 1, false);
                // Set a privious privilege mode for supervisor mode (SPP, 8) to 0.
                cpu.state.write_bit(SSTATUS, 8, true);
            }
            Mode::User => {
                // Set the program counter to the user trap-handler base address (utvec).
                cpu.pc = (cpu.state.read(UTVEC) & !1) as usize;

                cpu.state.write(UCAUSE, self.exception_code());
                cpu.state.write(UEPC, exception_pc);
                cpu.state.write(UTVAL, exception_pc);

                // TODO: implement to update USTATUS
            }
            _ => {}
        }

        match self {
            Exception::InstructionAddressMisaligned => Err(Exception::InstructionAddressMisaligned),
            Exception::InstructionAccessFault => Err(Exception::InstructionAccessFault),
            Exception::IllegalInstruction(s) => Err(Exception::IllegalInstruction(s.to_string())),
            Exception::Breakpoint => Err(Exception::Breakpoint),
            Exception::LoadAddressMisaligned => Err(Exception::LoadAddressMisaligned),
            Exception::LoadAccessFault => Err(Exception::LoadAccessFault),
            Exception::StoreAMOAddressMisaligned => Err(Exception::StoreAMOAddressMisaligned),
            Exception::StoreAMOAccessFault => Err(Exception::StoreAMOAccessFault),
            Exception::EnvironmentCallFromUMode => Ok(()),
            Exception::EnvironmentCallFromSMode => Ok(()),
            Exception::EnvironmentCallFromMMode => Ok(()),
            Exception::InstructionPageFault => Err(Exception::InstructionPageFault),
            Exception::LoadPageFault => Err(Exception::LoadPageFault),
            Exception::StoreAMOPageFault => Err(Exception::StoreAMOPageFault),
        }
    }
}
