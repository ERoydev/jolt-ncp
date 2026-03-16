use ckb_vm::memory::{fill_page_data, get_page_indices, set_dirty, Memory};
use ckb_vm::{Bytes, Error, RISCV_MAX_MEMORY, RISCV_PAGESIZE};

/// Custom CKB-VM memory backend for Jolt ZK-VM guests.
///
/// CKB-VM expects a zero-based 4 MB address space (0x0..0x3F_FFFF).
/// Inside Jolt, guest DRAM starts at 0x8000_0000, so raw pointer
/// arithmetic would fault on address 0x0.
///
/// `JoltMemory` side-steps this by treating CKB-VM addresses as
/// *indices* into a plain `Vec<u8>` that lives in Jolt's valid DRAM.
/// Every load/store uses `from_le_bytes` / `to_le_bytes` on byte
/// slices — no `Cursor`, no `byteorder` crate, and no `unsafe`.
pub struct JoltMemory {
    data: Vec<u8>,
    flags: Vec<u8>,
    memory_size: usize,
    riscv_pages: usize,
    load_reservation_address: u64,
}

impl Memory for JoltMemory {
    type REG = u64;

    fn new() -> Self {
        Self::new_with_memory(RISCV_MAX_MEMORY)
    }

    fn new_with_memory(memory_size: usize) -> Self {
        assert!(memory_size <= RISCV_MAX_MEMORY);
        assert!(memory_size % RISCV_PAGESIZE == 0);
        Self {
            data: vec![0; memory_size],
            flags: vec![0; memory_size / RISCV_PAGESIZE],
            memory_size,
            riscv_pages: memory_size / RISCV_PAGESIZE,
            load_reservation_address: u64::MAX,
        }
    }

    fn init_pages(
        &mut self,
        addr: u64,
        size: u64,
        _flags: u8,
        source: Option<Bytes>,
        offset_from_addr: u64,
    ) -> Result<(), Error> {
        fill_page_data(self, addr, size, source, offset_from_addr)
    }

    fn fetch_flag(&mut self, page: u64) -> Result<u8, Error> {
        if page < self.riscv_pages as u64 {
            Ok(self.flags[page as usize])
        } else {
            Err(Error::MemOutOfBound)
        }
    }

    fn set_flag(&mut self, page: u64, flag: u8) -> Result<(), Error> {
        if page < self.riscv_pages as u64 {
            self.flags[page as usize] |= flag;
            Ok(())
        } else {
            Err(Error::MemOutOfBound)
        }
    }

    fn clear_flag(&mut self, page: u64, flag: u8) -> Result<(), Error> {
        if page < self.riscv_pages as u64 {
            self.flags[page as usize] &= !flag;
            Ok(())
        } else {
            Err(Error::MemOutOfBound)
        }
    }

    fn memory_size(&self) -> usize {
        self.memory_size
    }

    // ── instruction-fetch helpers ───────────────────────────────────

    fn execute_load16(&mut self, addr: u64) -> Result<u16, Error> {
        self.load16(&addr).map(|v| v as u16)
    }

    fn execute_load32(&mut self, addr: u64) -> Result<u32, Error> {
        self.load32(&addr).map(|v| v as u32)
    }

    // ── byte-level loads (little-endian) ────────────────────────────

    fn load8(&mut self, addr: &u64) -> Result<u64, Error> {
        let a = *addr as usize;
        if a >= self.memory_size {
            return Err(Error::MemOutOfBound);
        }
        Ok(self.data[a] as u64)
    }

    fn load16(&mut self, addr: &u64) -> Result<u64, Error> {
        let a = *addr as usize;
        if a + 2 > self.memory_size {
            return Err(Error::MemOutOfBound);
        }
        let v = u16::from_le_bytes([self.data[a], self.data[a + 1]]);
        Ok(v as u64)
    }

    fn load32(&mut self, addr: &u64) -> Result<u64, Error> {
        let a = *addr as usize;
        if a + 4 > self.memory_size {
            return Err(Error::MemOutOfBound);
        }
        let v = u32::from_le_bytes([
            self.data[a],
            self.data[a + 1],
            self.data[a + 2],
            self.data[a + 3],
        ]);
        Ok(v as u64)
    }

    fn load64(&mut self, addr: &u64) -> Result<u64, Error> {
        let a = *addr as usize;
        if a + 8 > self.memory_size {
            return Err(Error::MemOutOfBound);
        }
        let v = u64::from_le_bytes([
            self.data[a],
            self.data[a + 1],
            self.data[a + 2],
            self.data[a + 3],
            self.data[a + 4],
            self.data[a + 5],
            self.data[a + 6],
            self.data[a + 7],
        ]);
        Ok(v)
    }

    // ── byte-level stores (little-endian) ───────────────────────────

    fn store8(&mut self, addr: &u64, value: &u64) -> Result<(), Error> {
        let a = *addr as usize;
        if a >= self.memory_size {
            return Err(Error::MemOutOfBound);
        }
        let page_indices = get_page_indices(*addr, 1)?;
        set_dirty(self, &page_indices)?;
        self.data[a] = *value as u8;
        Ok(())
    }

    fn store16(&mut self, addr: &u64, value: &u64) -> Result<(), Error> {
        let a = *addr as usize;
        if a + 2 > self.memory_size {
            return Err(Error::MemOutOfBound);
        }
        let page_indices = get_page_indices(*addr, 2)?;
        set_dirty(self, &page_indices)?;
        let bytes = (*value as u16).to_le_bytes();
        self.data[a] = bytes[0];
        self.data[a + 1] = bytes[1];
        Ok(())
    }

    fn store32(&mut self, addr: &u64, value: &u64) -> Result<(), Error> {
        let a = *addr as usize;
        if a + 4 > self.memory_size {
            return Err(Error::MemOutOfBound);
        }
        let page_indices = get_page_indices(*addr, 4)?;
        set_dirty(self, &page_indices)?;
        let bytes = (*value as u32).to_le_bytes();
        self.data[a] = bytes[0];
        self.data[a + 1] = bytes[1];
        self.data[a + 2] = bytes[2];
        self.data[a + 3] = bytes[3];
        Ok(())
    }

    fn store64(&mut self, addr: &u64, value: &u64) -> Result<(), Error> {
        let a = *addr as usize;
        if a + 8 > self.memory_size {
            return Err(Error::MemOutOfBound);
        }
        let page_indices = get_page_indices(*addr, 8)?;
        set_dirty(self, &page_indices)?;
        let bytes = value.to_le_bytes();
        self.data[a] = bytes[0];
        self.data[a + 1] = bytes[1];
        self.data[a + 2] = bytes[2];
        self.data[a + 3] = bytes[3];
        self.data[a + 4] = bytes[4];
        self.data[a + 5] = bytes[5];
        self.data[a + 6] = bytes[6];
        self.data[a + 7] = bytes[7];
        Ok(())
    }

    // ── bulk operations ─────────────────────────────────────────────

    fn store_bytes(&mut self, addr: u64, value: &[u8]) -> Result<(), Error> {
        let size = value.len();
        if size == 0 {
            return Ok(());
        }
        let a = addr as usize;
        if a + size > self.memory_size {
            return Err(Error::MemOutOfBound);
        }
        let page_indices = get_page_indices(addr, size as u64)?;
        set_dirty(self, &page_indices)?;
        self.data[a..a + size].copy_from_slice(value);
        Ok(())
    }

    fn store_byte(&mut self, addr: u64, size: u64, value: u8) -> Result<(), Error> {
        if size == 0 {
            return Ok(());
        }
        let a = addr as usize;
        let s = size as usize;
        if a + s > self.memory_size {
            return Err(Error::MemOutOfBound);
        }
        let page_indices = get_page_indices(addr, size)?;
        set_dirty(self, &page_indices)?;
        self.data[a..a + s].fill(value);
        Ok(())
    }

    fn load_bytes(&mut self, addr: u64, size: u64) -> Result<Bytes, Error> {
        if size == 0 {
            return Ok(Bytes::new());
        }
        let a = addr as usize;
        let s = size as usize;
        if a + s > self.memory_size {
            return Err(Error::MemOutOfBound);
        }
        Ok(Bytes::from(self.data[a..a + s].to_vec()))
    }

    // ── atomic load-reservation ─────────────────────────────────────

    fn lr(&self) -> &u64 {
        &self.load_reservation_address
    }

    fn set_lr(&mut self, value: &u64) {
        self.load_reservation_address = *value;
    }
}
