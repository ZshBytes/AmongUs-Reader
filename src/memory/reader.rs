use crate::memory::error::MemoryError;
use crate::memory::process::ProcessHandle;

pub struct MemoryReader<'a> {
    process: &'a ProcessHandle,
}

impl<'a> MemoryReader<'a> {
    pub fn new(process: &'a ProcessHandle) -> Self {
        Self { process }
    }

    pub fn process(&self) -> &ProcessHandle {
        self.process
    }

    pub fn read_u8(&self, address: u64) -> Result<u8, MemoryError> {
        let mut buf = [0u8; 1];
        self.process.read_raw(address, &mut buf)?;
        Ok(buf[0])
    }

    pub fn read_i32(&self, address: u64) -> Result<i32, MemoryError> {
        let mut buf = [0u8; 4];
        self.process.read_raw(address, &mut buf)?;
        Ok(i32::from_le_bytes(buf))
    }

    pub fn read_u16(&self, address: u64) -> Result<u16, MemoryError> {
        let mut buf = [0u8; 2];
        self.process.read_raw(address, &mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }

    pub fn read_u64(&self, address: u64) -> Result<u64, MemoryError> {
        let mut buf = [0u8; 8];
        self.process.read_raw(address, &mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }

    #[allow(dead_code)]
    pub fn read_bool(&self, address: u64) -> Result<bool, MemoryError> {
        Ok(self.read_u8(address)? != 0)
    }

    pub fn read_pointer(&self, address: u64) -> Result<u64, MemoryError> {
        let ptr = match self.process.pointer_size() {
            4 => {
                let mut buf = [0u8; 4];
                self.process.read_raw(address, &mut buf)?;
                u32::from_le_bytes(buf) as u64
            }
            8 => self.read_u64(address)?,
            _ => self.read_u64(address)?,
        };

        if ptr == 0 {
            return Err(MemoryError::InvalidPointer(0));
        }
        if !self.process.is_valid_pointer(ptr) {
            return Err(MemoryError::InvalidPointer(ptr));
        }
        Ok(ptr)
    }

    pub fn read_bytes(&self, address: u64, buffer: &mut [u8]) -> Result<(), MemoryError> {
        self.process.read_raw(address, buffer)
    }
}
