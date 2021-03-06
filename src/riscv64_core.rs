use crate::riscv_csr::CsrAddr;
use crate::riscv_csr::RiscvCsr;

use crate::riscv_mmu::RiscvMmu;

use crate::riscv32_core::PrivMode;
use crate::riscv32_core::VMMode;

use crate::riscv32_core::MemAccType;
use crate::riscv32_core::MemResult;

use crate::riscv_tracer::TraceInfo;
use crate::riscv_tracer::TraceType;
use crate::riscv_tracer::Tracer;

use crate::riscv32_core::AddrT;
use crate::riscv32_core::InstT;
use crate::riscv32_core::RegAddrT;
use crate::riscv32_core::XlenT;

use crate::riscv32_core::DRAM_BASE;
use crate::riscv32_core::DRAM_SIZE;

use crate::riscv_csr_bitdef::SYSREG_SATP_MODE_LSB;
use crate::riscv_csr_bitdef::SYSREG_SATP_MODE_MSB;

pub type Xlen64T = i64;
pub type UXlen64T = u64;

pub struct Riscv64Env {
    // m_bitmode: RiscvBitMode,
    pub m_pc: AddrT,
    m_previous_pc: AddrT,
    m_regs: [Xlen64T; 32],
    pub m_memory: [u8; DRAM_SIZE], // memory
    pub m_csr: RiscvCsr<Xlen64T>,

    pub m_priv: PrivMode,
    m_maxpriv: PrivMode,
    m_vmmode: VMMode,

    pub m_trace: Tracer,

    m_tohost_addr: AddrT,
    m_fromhost_addr: AddrT,
    m_tohost: Xlen64T,
    m_fromhost: Xlen64T,

    m_finish_cpu: bool,

    m_is_update_pc: bool,
}

impl Riscv64Env {
    pub fn new() -> Riscv64Env {
        Riscv64Env {
            // m_bitmode: RiscvBitMode::Bit32,
            m_pc: DRAM_BASE as AddrT,
            m_memory: [0; DRAM_SIZE],
            m_regs: [0; 32],
            m_maxpriv: PrivMode::Machine,
            m_previous_pc: 0,
            m_vmmode: VMMode::Mbare,
            m_finish_cpu: false,
            m_fromhost_addr: (DRAM_BASE + 0x001000) as AddrT,
            m_tohost_addr: (DRAM_BASE + 0x001000) as AddrT,
            m_fromhost: 0,
            m_tohost: 0,
            m_is_update_pc: false,
            m_csr: RiscvCsr::<i64>::new(),
            m_priv: PrivMode::Machine,
            m_trace: Tracer::new(),
        }
    }

    pub fn extend_sign(data: Xlen64T, msb: Xlen64T) -> Xlen64T {
        let mask: Xlen64T = 1 << msb; // mask can be pre-computed if b is fixed
        let res_data = data & ((1 << (msb + 1)) - 1); // (Skip this if bits in x above position b are already zero.)
        return (res_data ^ mask) - mask;
    }

    pub fn extract_bit_field(hex: Xlen64T, left: u8, right: u8) -> Xlen64T {
        let mask: Xlen64T = (1 << (left - right + 1)) - 1;
        return (hex >> right) & mask;
    }

    pub fn set_bit_field(hex: Xlen64T, val: Xlen64T, left: u8, right: u8) -> Xlen64T {
        let mask: Xlen64T = (1 << (left - right + 1)) - 1;
        return (hex & !(mask << right)) | (val << right);
    }

    pub fn extract_uj_field(hex: InstT) -> Xlen64T {
        let i24_21 = Self::extract_bit_field(hex as Xlen64T, 24, 21) & 0x0f;
        let i30_25 = Self::extract_bit_field(hex as Xlen64T, 30, 25) & 0x03f;
        let i20_20 = Self::extract_bit_field(hex as Xlen64T, 20, 20) & 0x01;
        let i19_12 = Self::extract_bit_field(hex as Xlen64T, 19, 12) & 0x0ff;
        let i31_31 = Self::extract_bit_field(hex as Xlen64T, 31, 31) & 0x01;

        let u_res: Xlen64T =
            (i31_31 << 20) | (i19_12 << 12) | (i20_20 << 11) | (i30_25 << 5) | (i24_21 << 1);
        return Self::extend_sign(u_res, 20);
    }

    pub fn extract_ifield(hex: InstT) -> Xlen64T {
        let uimm32: Xlen64T = Self::extract_bit_field(hex as Xlen64T, 31, 20);
        return Self::extend_sign(uimm32, 11);
    }

    pub fn extract_shamt_field(hex: InstT) -> Xlen64T {
        return Self::extract_bit_field(hex as Xlen64T, 25, 20);
    }

    pub fn extract_sb_field(hex: InstT) -> Xlen64T {
        let i07_07: Xlen64T = Self::extract_bit_field(hex as Xlen64T, 7, 7) & 0x01;
        let i11_08: Xlen64T = Self::extract_bit_field(hex as Xlen64T, 11, 8) & 0x0f;
        let i30_25: Xlen64T = Self::extract_bit_field(hex as Xlen64T, 30, 25) & 0x03f;
        let i31_31: Xlen64T = Self::extract_bit_field(hex as Xlen64T, 31, 31) & 0x01;

        let u_res: Xlen64T = (i31_31 << 12) | (i07_07 << 11) | (i30_25 << 5) | (i11_08 << 1);
        return Self::extend_sign(u_res, 12);
    }

    pub fn extract_sfield(hex: InstT) -> Xlen64T {
        let i11_07: Xlen64T = Self::extract_bit_field(hex as Xlen64T, 11, 7) & 0x01f;
        let i31_25: Xlen64T = Self::extract_bit_field(hex as Xlen64T, 31, 25) & 0x07f;

        let u_res: Xlen64T = (i31_25 << 5) | (i11_07 << 0);

        return Self::extend_sign(u_res, 11);
    }

    fn sext_xlen(hex: InstT) -> Xlen64T {
        return hex as Xlen64T;
    }
    fn uext_xlen(hex: InstT) -> UXlen64T {
        return hex as UXlen64T;
    }

    pub fn is_update_pc(&mut self) -> bool {
        return self.m_is_update_pc;
    }
    pub fn set_update_pc(&mut self, update_pc: bool) {
        self.m_is_update_pc = update_pc;
    }
}

pub trait Riscv64Core {
    fn get_rs1_addr(inst: InstT) -> RegAddrT;
    fn get_rs2_addr(inst: InstT) -> RegAddrT;
    fn get_rd_addr(inst: InstT) -> RegAddrT;

    fn set_pc(&mut self, addr: AddrT);
    fn get_pc(&mut self) -> AddrT;
    fn get_previous_pc(&mut self) -> AddrT;

    fn read_memory_dword(&mut self, phy_addr: AddrT) -> Xlen64T;
    fn read_memory_word(&mut self, phy_addr: AddrT) -> Xlen64T;
    fn read_memory_hword(&mut self, phy_addr: AddrT) -> Xlen64T;
    fn read_memory_byte(&mut self, phy_addr: AddrT) -> Xlen64T;
    fn write_memory_dword(&mut self, phy_addr: AddrT, data: Xlen64T) -> Xlen64T;
    fn write_memory_word(&mut self, phy_addr: AddrT, data: Xlen64T) -> Xlen64T;
    fn write_memory_hword(&mut self, phy_addr: AddrT, data: Xlen64T) -> Xlen64T;
    fn write_memory_byte(&mut self, phy_addr: AddrT, data: Xlen64T) -> Xlen64T;

    fn fetch_bus(&mut self) -> (MemResult, InstT);
    fn read_bus_dword(&mut self, addr: AddrT) -> (MemResult, Xlen64T);
    fn read_bus_word(&mut self, addr: AddrT) -> (MemResult, Xlen64T);
    fn read_bus_hword(&mut self, addr: AddrT) -> (MemResult, Xlen64T);
    fn read_bus_byte(&mut self, addr: AddrT) -> (MemResult, Xlen64T);
    fn write_bus_dword(&mut self, addr: AddrT, data: Xlen64T) -> MemResult;
    fn write_bus_word(&mut self, addr: AddrT, data: Xlen64T) -> MemResult;
    fn write_bus_hword(&mut self, addr: AddrT, data: Xlen64T) -> MemResult;
    fn write_bus_byte(&mut self, addr: AddrT, data: Xlen64T) -> MemResult;

    fn read_reg(&mut self, reg_addr: RegAddrT) -> Xlen64T;
    fn write_reg(&mut self, reg_addr: RegAddrT, data: Xlen64T);

    // fn decode_inst(&mut self, inst: InstT) -> RiscvInstId;
    // fn execute_inst(&mut self, dec_inst: RiscvInstId, inst: InstT, step: u32);

    // fn print_priv_mode(priv_mode: PrivMode) -> &str;

    // fn get_priv_mode(&mut self) -> PrivMode;
    fn set_priv_mode(&mut self, priv_mode: PrivMode);

    // fn get_max_priv(&mut self) -> PrivMode;
    fn set_max_priv(&mut self, maxpriv: PrivMode);

    fn get_vm_mode(&mut self) -> VMMode;
    fn set_vm_mode(&mut self, vmmode: VMMode);

    fn get_is_finish_cpu(&mut self) -> bool;

    fn get_tohost(&mut self) -> Xlen64T;
    fn get_fromhost(&mut self) -> Xlen64T;
}

impl Riscv64Core for Riscv64Env {
    fn get_rs1_addr(inst: InstT) -> RegAddrT {
        return ((inst >> 15) & 0x1f) as RegAddrT;
    }
    fn get_rs2_addr(inst: InstT) -> RegAddrT {
        return ((inst >> 20) & 0x1f) as RegAddrT;
    }
    fn get_rd_addr(inst: InstT) -> RegAddrT {
        return ((inst >> 7) & 0x1f) as RegAddrT;
    }

    fn read_reg(&mut self, reg_addr: RegAddrT) -> Xlen64T {
        let ret_val: Xlen64T;

        if reg_addr == 0 {
            ret_val = 0;
        } else {
            ret_val = self.m_regs[reg_addr as usize];
        }

        let mut read_reg_trace = TraceInfo::new();
        read_reg_trace.m_trace_type = TraceType::XRegRead;
        read_reg_trace.m_trace_addr = reg_addr as AddrT;
        read_reg_trace.m_trace_value = ret_val;
        read_reg_trace.m_trace_memresult = MemResult::NoExcept;

        self.m_trace.m_trace_info.push(read_reg_trace);

        return ret_val;
    }

    fn write_reg(&mut self, reg_addr: RegAddrT, data: Xlen64T) {
        if reg_addr != 0 {
            let mut write_reg_trace = TraceInfo::new();

            write_reg_trace.m_trace_type = TraceType::XRegWrite;
            write_reg_trace.m_trace_addr = reg_addr as AddrT;
            write_reg_trace.m_trace_value = data;
            write_reg_trace.m_trace_memresult = MemResult::NoExcept;

            self.m_trace.m_trace_info.push(write_reg_trace);

            self.m_regs[reg_addr as usize] = data;
            // println!("     x{:02} <= {:08x}", reg_addr, data);
        }
    }

    fn set_pc(&mut self, addr: AddrT) {
        self.m_previous_pc = self.m_pc;
        self.m_pc = addr;
    }

    fn get_pc(&mut self) -> AddrT {
        return self.m_pc;
    }

    fn get_previous_pc(&mut self) -> AddrT {
        return self.m_previous_pc;
    }

    // fn get_priv_mode(&mut self) -> PrivMode {
    //     return self.m_priv;
    // }
    fn set_priv_mode(&mut self, priv_mode: PrivMode) {
        self.m_priv = priv_mode;
        // FlushTlb();
    }

    // fn get_max_priv(&mut self) -> PrivMode {
    //     return self.m_maxpriv;
    // }

    fn set_max_priv(&mut self, maxpriv: PrivMode) {
        self.m_maxpriv = maxpriv;
    }

    fn set_vm_mode(&mut self, vmmode: VMMode) {
        self.m_vmmode = vmmode;
    }

    fn read_memory_dword(&mut self, phy_addr: AddrT) -> Xlen64T {
        let dword_val: Xlen64T = (self.read_memory_byte(phy_addr + 7) << 56)
            | (self.read_memory_byte(phy_addr + 6) << 48)
            | (self.read_memory_byte(phy_addr + 5) << 40)
            | (self.read_memory_byte(phy_addr + 4) << 32)
            | (self.read_memory_byte(phy_addr + 3) << 24)
            | (self.read_memory_byte(phy_addr + 2) << 16)
            | (self.read_memory_byte(phy_addr + 1) << 8)
            | (self.read_memory_byte(phy_addr + 0) << 0);
        return dword_val;
    }

    fn read_memory_word(&mut self, phy_addr: AddrT) -> Xlen64T {
        if phy_addr == self.m_tohost_addr {
            return self.m_tohost;
        } else if phy_addr == self.m_fromhost_addr {
            return self.m_fromhost;
        } else {
            let word_val: XlenT = ((self.read_memory_byte(phy_addr + 3) << 24) as XlenT)
                | ((self.read_memory_byte(phy_addr + 2) << 16) as XlenT)
                | ((self.read_memory_byte(phy_addr + 1) << 8) as XlenT)
                | ((self.read_memory_byte(phy_addr + 0) << 0) as XlenT);
            return word_val as Xlen64T;
        }
    }

    fn read_memory_hword(&mut self, phy_addr: AddrT) -> Xlen64T {
        return (self.read_memory_byte(phy_addr + 1) << 8)
            | (self.read_memory_byte(phy_addr + 0) << 0);
    }

    fn read_memory_byte(&mut self, phy_addr: AddrT) -> Xlen64T {
        assert!(phy_addr >= DRAM_BASE);
        let base_addr: AddrT = phy_addr - DRAM_BASE;

        return self.m_memory[base_addr as usize + 0] as Xlen64T;
    }

    fn write_memory_dword(&mut self, phy_addr: AddrT, data: Xlen64T) -> Xlen64T {
        self.write_memory_byte(phy_addr + 0, (data >> 0) & 0xff);
        self.write_memory_byte(phy_addr + 1, (data >> 8) & 0xff);
        self.write_memory_byte(phy_addr + 2, (data >> 16) & 0xff);
        self.write_memory_byte(phy_addr + 3, (data >> 24) & 0xff);
        self.write_memory_byte(phy_addr + 4, (data >> 32) & 0xff);
        self.write_memory_byte(phy_addr + 5, (data >> 40) & 0xff);
        self.write_memory_byte(phy_addr + 6, (data >> 48) & 0xff);
        self.write_memory_byte(phy_addr + 7, (data >> 56) & 0xff);

        return 0;
    }

    fn write_memory_word(&mut self, phy_addr: AddrT, data: Xlen64T) -> Xlen64T {
        if phy_addr == self.m_tohost_addr {
            self.m_finish_cpu = true;
            self.m_tohost = data;
        } else if phy_addr == self.m_fromhost_addr {
            self.m_finish_cpu = true;
            self.m_fromhost = data;
        } else {
            self.write_memory_byte(phy_addr + 0, (data >> 0) & 0xff);
            self.write_memory_byte(phy_addr + 1, (data >> 8) & 0xff);
            self.write_memory_byte(phy_addr + 2, (data >> 16) & 0xff);
            self.write_memory_byte(phy_addr + 3, (data >> 24) & 0xff);
        }
        return 0;
    }

    fn write_memory_hword(&mut self, phy_addr: AddrT, data: Xlen64T) -> Xlen64T {
        self.write_memory_byte(phy_addr + 0, (data >> 0) & 0xff);
        self.write_memory_byte(phy_addr + 1, (data >> 8) & 0xff);

        return 0;
    }

    fn write_memory_byte(&mut self, phy_addr: AddrT, data: Xlen64T) -> Xlen64T {
        assert!(phy_addr >= DRAM_BASE);
        let base_addr: AddrT = phy_addr - DRAM_BASE;

        self.m_memory[base_addr as usize] = (data & 0xff) as u8;
        return 0;
    }

    fn fetch_bus(&mut self) -> (MemResult, InstT) {
        // let result: MemResult;
        // let phy_addr: AddrT;
        let (result, phy_addr) = self.convert_virtual_address(self.m_pc, MemAccType::Fetch);

        if result != MemResult::NoExcept {
            return (result, 0);
        }
        return (result, self.read_memory_word(phy_addr) as InstT);
    }

    fn read_bus_dword(&mut self, addr: AddrT) -> (MemResult, Xlen64T) {
        let (result, phy_addr) = self.convert_virtual_address(addr, MemAccType::Read);

        if result != MemResult::NoExcept {
            return (result, 0);
        }

        let ret_val = self.read_memory_dword(phy_addr);

        let mut read_mem_trace = TraceInfo::new();

        read_mem_trace.m_trace_type = TraceType::MemRead;
        read_mem_trace.m_trace_addr = addr;
        read_mem_trace.m_trace_value = ret_val;
        read_mem_trace.m_trace_memresult = MemResult::NoExcept;

        self.m_trace.m_trace_info.push(read_mem_trace);

        return (result, ret_val);
    }

    fn read_bus_word(&mut self, addr: AddrT) -> (MemResult, Xlen64T) {
        let (result, phy_addr) = self.convert_virtual_address(addr, MemAccType::Read);

        if result != MemResult::NoExcept {
            return (result, 0);
        }

        let ret_val = self.read_memory_word(phy_addr);

        let mut read_mem_trace = TraceInfo::new();

        read_mem_trace.m_trace_type = TraceType::MemRead;
        read_mem_trace.m_trace_addr = addr;
        read_mem_trace.m_trace_value = ret_val;
        read_mem_trace.m_trace_memresult = MemResult::NoExcept;

        self.m_trace.m_trace_info.push(read_mem_trace);

        return (result, ret_val);
    }

    fn read_bus_hword(&mut self, addr: AddrT) -> (MemResult, Xlen64T) {
        let (result, phy_addr) = self.convert_virtual_address(addr, MemAccType::Read);

        if result != MemResult::NoExcept {
            return (result, 0);
        }

        return (result, self.read_memory_hword(phy_addr));
    }

    fn read_bus_byte(&mut self, addr: AddrT) -> (MemResult, Xlen64T) {
        let (result, phy_addr) = self.convert_virtual_address(addr, MemAccType::Read);

        if result != MemResult::NoExcept {
            return (result, 0);
        }
        return (result, self.read_memory_byte(phy_addr));
    }

    fn write_bus_dword(&mut self, addr: AddrT, data: Xlen64T) -> MemResult {
        // let result: MemResult;
        // let phy_addr: AddrT;
        let (result, phy_addr) = self.convert_virtual_address(addr, MemAccType::Write);

        if result != MemResult::NoExcept {
            return result;
        }

        let mut write_mem_trace = TraceInfo::new();

        write_mem_trace.m_trace_type = TraceType::MemWrite;
        write_mem_trace.m_trace_addr = addr;
        write_mem_trace.m_trace_value = data;
        write_mem_trace.m_trace_memresult = MemResult::NoExcept;

        self.m_trace.m_trace_info.push(write_mem_trace);

        self.write_memory_dword(phy_addr, data);

        return result;
    }

    fn write_bus_word(&mut self, addr: AddrT, data: Xlen64T) -> MemResult {
        // let result: MemResult;
        // let phy_addr: AddrT;
        let (result, phy_addr) = self.convert_virtual_address(addr, MemAccType::Write);

        if result != MemResult::NoExcept {
            return result;
        }

        let mut write_mem_trace = TraceInfo::new();

        write_mem_trace.m_trace_type = TraceType::MemWrite;
        write_mem_trace.m_trace_addr = addr;
        write_mem_trace.m_trace_value = data;
        write_mem_trace.m_trace_memresult = MemResult::NoExcept;

        self.m_trace.m_trace_info.push(write_mem_trace);

        self.write_memory_word(phy_addr, data);

        return result;
    }

    fn write_bus_hword(&mut self, addr: AddrT, data: Xlen64T) -> MemResult {
        let (result, phy_addr) = self.convert_virtual_address(addr, MemAccType::Write);

        if result != MemResult::NoExcept {
            return result;
        }

        self.write_memory_hword(phy_addr, data);

        return result;
    }

    fn write_bus_byte(&mut self, addr: AddrT, data: Xlen64T) -> MemResult {
        let (result, phy_addr) = self.convert_virtual_address(addr, MemAccType::Write);

        if result != MemResult::NoExcept {
            return result;
        }

        self.write_memory_byte(phy_addr, data);

        return result;
    }

    fn get_vm_mode(&mut self) -> VMMode {
        let satp_val = self.m_csr.csrrs(CsrAddr::Satp, 0) as Xlen64T; // SATP;
        let mode = Self::extract_bit_field(satp_val, SYSREG_SATP_MODE_MSB, SYSREG_SATP_MODE_LSB);
        return if mode == 1 {
            VMMode::Sv32
        } else {
            VMMode::Mbare
        };
    }

    // fn print_priv_mode(priv_mode: PrivMode) -> &str {
    //     return match priv_mode {
    //         PrivMode::User => "UserMode",
    //         PrivMode::Supervisor => "SuprevisorMode",
    //         PrivMode::Hypervisor => "HypervisorMode",
    //         PrivMode::Machine => "MachineMode",
    //         _ => "<Internal Error: PrivMode is illegal>",
    //     };
    // }

    fn get_is_finish_cpu(&mut self) -> bool {
        return self.m_finish_cpu;
    }

    fn get_tohost(&mut self) -> Xlen64T {
        return self.m_tohost;
    }
    fn get_fromhost(&mut self) -> Xlen64T {
        return self.m_fromhost;
    }
}
