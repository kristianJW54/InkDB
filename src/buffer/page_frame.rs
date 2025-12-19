// The buffer manages page frames
//
//

pub(crate) struct PageFrame {
    checksum: u32,
    page: SlottedPage,
}
