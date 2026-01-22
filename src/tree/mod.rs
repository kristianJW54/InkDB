pub mod btree;
pub mod btree_base;

pub(crate) type CompareFn = fn(&[u8], &[u8]) -> std::cmp::Ordering;
