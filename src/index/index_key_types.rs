// For a key type in an index column - ascociated operator classes for that type can be made
// Each operator class will define support functions one of which include a comparator function for semantic comparison
// The index type of the operator class will define the support functions of the operator class
// For our simple implementation we will only be providing a b-tree operator class and later on highlight the need to plug into a wider operator family through
// a stub.

use std::cmp::Ordering;

pub(crate) type CompareFn = fn(&[u8], &[u8]) -> std::cmp::Ordering;

pub(crate) trait KeyType {}
// Int8
// Int16
// Int32... for index column types

struct Int32;
impl KeyType for Int32 {}
struct Int64;
impl KeyType for Int64 {}

pub(crate) trait BTreeOpClass {
    type KeyType: KeyType;

    fn basic_comp(&self) -> CompareFn;
    // More support functions can be added here
}

struct Int32Operator;

impl BTreeOpClass for Int32Operator {
    type KeyType = Int32;
    fn basic_comp(&self) -> CompareFn {
        |a, b| {
            let a = i32::from_le_bytes(a.try_into().unwrap());
            let b = i32::from_le_bytes(b.try_into().unwrap());
            a.cmp(&b)
        }
    }
}

#[test]
fn op_stuff() {
    let int32 = Int32;

    let op_class32 = Int32Operator;

    // Lets try and compare some numbers

    let a: i32 = 10;
    let b: i32 = 20;

    // Now can we store this? Multiple times?
    let function: CompareFn = op_class32.basic_comp();
    let function_again: CompareFn = op_class32.basic_comp();
    println!("result {:?}", function(&a.to_le_bytes(), &b.to_le_bytes()));
    println!(
        "result {:?}",
        function_again(&a.to_le_bytes(), &b.to_le_bytes())
    );
}
