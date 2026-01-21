// TODO: Replace
pub(crate) type CompareFn = fn(&[u8], &[u8]) -> std::cmp::Ordering;

// For a key type in an index column - ascociated operator classes for that type can be made
// Each operator class will define support functions one of which include a comparator function for semantic comparison
// The index type of the operator class will define the support functions of the operator class
// For our simple implementation we will only be providing a b-tree operator class and later on highlight the need to plug into a wider operator family through
// a stub.

// A key should ALWAYS be encoded before being used in an index structure or stored
//
// Type state ensures that only canonically encoded keys can reach the B-tree,
// and the operator binds encoding and ordering into a single, inlinable unit.
//

use crate::page::SlottedPageRef;
use crate::page::index_cell::IndexCellRef;
use std::cmp::Ordering;
use std::marker::PhantomData;

// TODO: Need to finish this and integrate with the rest of the codebase
// TODO: Need to think about how we want to move SearchKey<Raw> into SearchKey<Encoded>
// TODO: Need to think about how this interacts with SlottedPageRef and how the operator class is stored in the b-tree

// States
pub(crate) struct Raw {}
pub(crate) struct Encoded {}
pub(crate) struct Decoded {}

// Define a key type for index column types
pub(crate) trait KeyType {}

// Define an Operator Class for key types to implement
pub(crate) trait OperatorClass: Sized {
    type Value;
    type KeyType: KeyType;

    const WIDTH: usize;

    fn encode(value: &Self::Value, out: &mut [u8]);

    fn compare_search_key<'a>(
        a: SearchKeyType<'a, Self, Encoded>,
        b: PageKeyType<'a, Self>,
    ) -> Ordering;

    fn compare_page_key<'a>(a: PageKeyType<'_, Self>, b: PageKeyType<'_, Self>) -> Ordering;
}

//
// PageKey is always encoded implicitly as it is stored in the page and must be encoded
pub(crate) struct PageKeyType<'a, O: OperatorClass> {
    cell: IndexCellRef<'a>,
    _operator: PhantomData<O>,
}

pub(crate) struct SearchKeyType<'a, O: OperatorClass, S> {
    key: &'a [u8],
    _state: PhantomData<S>,
    _operator: PhantomData<O>,
}

// Test with i32
