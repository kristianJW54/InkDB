


pub(crate) struct RawPage{
    data: [u8; 4096],
}

impl RawPage {
    pub fn write(&mut self, bytes: &mut [u8]) -> Result<(),()> {
        self.data.copy_from_slice(bytes);
        Ok(())
    }
}

// We need two page types initially, Heap and Index.
// We will need a header, slot array and cell
// The cell will have a header for transactions and row data