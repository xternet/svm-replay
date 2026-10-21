//! Little-endian reader over the instruction data.
use solana_program::program_error::ProgramError;

pub(crate) struct Reader<'a> {
    pub(crate) data: &'a [u8],
    pub(crate) at: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn bytes(&mut self, n: usize) -> Result<&'a [u8], ProgramError> {
        let out = self
            .data
            .get(self.at..self.at + n)
            .ok_or(ProgramError::InvalidInstructionData)?;
        self.at += n;
        Ok(out)
    }
    pub(crate) fn u8(&mut self) -> Result<u8, ProgramError> {
        Ok(self.bytes(1)?[0])
    }
    pub(crate) fn u16(&mut self) -> Result<u16, ProgramError> {
        Ok(u16::from_le_bytes(self.bytes(2)?.try_into().unwrap()))
    }
    pub(crate) fn u32(&mut self) -> Result<u32, ProgramError> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }
    pub(crate) fn u64(&mut self) -> Result<u64, ProgramError> {
        Ok(u64::from_le_bytes(self.bytes(8)?.try_into().unwrap()))
    }
}
