// I want to implement a prefix compression algorithm for page specific layers to implement which uses core slotted page maniuplation to
// track, store and manage prefix offsets and compressed keys

// TODO: Implement a trait for prefix compression
// TODO: Need to think about at what level does prefix compression occur and how is it implemented

pub(crate) trait PxCompression {
    // TODO: Think about if we need a default/blanket implementation that is shared across types?
    // fn compress(&self, key: [u8]) -> [u8];
    fn print(&self);
}

struct FakePage {
    // Page will hold compressed keys - the first key will be a full key and the rest will be compressed
    p: [u8; 100],
}

impl<'a> FakePage {
    fn new() -> Self {
        FakePage { p: [0; 100] }
    }
}

#[test]
fn test_prefix_compression() {
    let page = FakePage::new();
    println!("{:?}", page.p);
}
