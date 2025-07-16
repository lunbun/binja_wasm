use binaryninja::binary_view::{BinaryView, BinaryViewBase};

pub trait BinaryReadable {
    fn read_u32_leb128(&self, addr: u64) -> Result<(u32, u8), ()>;
}

impl BinaryReadable for BinaryView {
    fn read_u32_leb128(&self, addr: u64) -> Result<(u32, u8), ()> {
        let mut buf = [0u8; 5];
        let n_read = self.read(&mut buf, addr);
        let buf = &buf[..n_read];
        let mut result = 0u32;
        let mut shift = 0u8;
        let mut n_bytes = 0u8;
        for &byte in buf {
            result |= ((byte & 0x7f) as u32) << shift;
            n_bytes += 1;
            if byte & 0x80 == 0 {
                return Ok((result, n_bytes));
            }
            shift += 7;
        }
        Err(())
    }
}
