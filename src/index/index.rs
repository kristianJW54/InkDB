// Index coordinates different b-tree (index structures) on disk
// it manages meta data to use when calling into indexes

pub(crate) struct Index<E: OperatorEncoding> {
    encoding: E,
    root_page_id: u64, // PAGE ID
                       // More
}
