// I want to implement a prefix compression algorithm for page specific layers to implement which uses core slotted page maniuplation to
// track, store and manage prefix offsets and compressed keys

// TODO: Build prefix compression functions which can be used by the page specific layers to implement prefix compression

// First funciton needs to take a new key to be inserted and get a local offset from the reference key

use std::iter;

use crate::page::key_view::KeyView;

// Function to calculate the longest common prefix of two byte slices
// https://users.rust-lang.org/t/how-to-find-common-prefix-of-two-byte-slices-effectively/25815/3
//
fn lcp_calculate<const N: usize>(x: &[u8], y: &[u8]) -> usize {
    let offset = iter::zip(x.chunks_exact(N), y.chunks_exact(N))
        .take_while(|(x, y)| x == y)
        .count()
        * N;
    // For the compiler optimisation to work on chunks exact we need to deal with any remainder left over
    offset
        + iter::zip(&x[offset..], &y[offset..])
            .take_while(|(x, y)| x == y)
            .count()
}

fn lcp_32(x: &[u8], y: &[u8]) -> usize {
    lcp_calculate::<32>(x, y)
}

fn lcp_64(x: &[u8], y: &[u8]) -> usize {
    lcp_calculate::<64>(x, y)
}

fn lcp_128(x: &[u8], y: &[u8]) -> usize {
    lcp_calculate::<128>(x, y)
}

fn lcp_256(x: &[u8], y: &[u8]) -> usize {
    lcp_calculate::<256>(x, y)
}

pub(super) fn find_prefix_offset(reference_key: &[u8], source_key: &[u8]) -> usize {
    let len = reference_key.len();
    if len < 32 {
        lcp_32(reference_key, source_key)
    } else if len < 64 {
        lcp_64(reference_key, source_key)
    } else if len < 128 {
        lcp_128(reference_key, source_key)
    } else if len < 256 {
        lcp_256(reference_key, source_key)
    } else {
        lcp_calculate::<256>(reference_key, source_key)
    }
}

pub(super) fn common_prefix_len(ref_key: &[u8], source_key: KeyView<'_>) -> usize {
    let max = std::cmp::min(ref_key.len(), source_key.len());
    let mut i = 0;
    while i < max && ref_key[i] == source_key.byte_at(i) {
        i += 1;
    }
    i
}

#[test]
fn prefix_test() {
    let long_key = "00000000000000000000000000000000000000000123".as_bytes();
    let compare_key = "00000000000000000000000000000000000000000423".as_bytes();
    let offset = find_prefix_offset(long_key, compare_key);
    assert_eq!(offset, 41);
    assert_eq!(long_key[offset], 0x73);
}
