use std::cmp::Ordering;

//

#[derive(Debug)]
pub(crate) struct KeyView<'a> {
    pub(crate) prefix: &'a [u8],
    pub(crate) suffix: &'a [u8],
}

impl KeyView<'_> {
    #[inline]
    pub(super) fn len(&self) -> usize {
        self.prefix.len() + self.suffix.len()
    }

    #[inline]
    pub(super) fn byte_at(&self, i: usize) -> u8 {
        if i < self.prefix.len() {
            self.prefix[i]
        } else {
            self.suffix[i - self.prefix.len()]
        }
    }
}

pub(super) fn cmp_p2p(a: &KeyView<'_>, b: &KeyView<'_>) -> Ordering {
    // If both keys have no prefix, compare suffixes
    if a.prefix.len() == 0 && b.prefix.len() == 0 {
        return cmp_suffix(a, b);
    }

    let mut i = 0;

    while i < a.prefix.len() && i < b.prefix.len() {
        let ord = a.prefix[i].cmp(&b.prefix[i]);
        if ord != Ordering::Equal {
            return ord;
        }
        i += 1;
    }

    if a.prefix.len() != b.prefix.len() {
        return a.prefix.len().cmp(&b.prefix.len());
    }

    // Compare suffixes

    let mut j = 0;

    while j < a.suffix.len() && j < b.suffix.len() {
        let ord = a.suffix[j].cmp(&b.suffix[j]);
        if ord != Ordering::Equal {
            return ord;
        }
        j += 1;
    }

    a.suffix.len().cmp(&b.suffix.len())
}

// TODO: Create compare search key &[u8] with KeyView function

fn cmp_suffix(a: &KeyView<'_>, b: &KeyView<'_>) -> Ordering {
    let mut i = 0;

    while i < a.suffix.len() && i < b.suffix.len() {
        let ord = a.suffix[i].cmp(&b.suffix[i]);
        if ord != Ordering::Equal {
            return ord;
        }
        i += 1;
    }

    a.suffix.len().cmp(&b.suffix.len())
}

pub(super) fn cmp_search(a: &[u8], b: KeyView<'_>) -> Ordering {
    // The search key is not compressed but is encoded so we can do bytewise comparison of the key against the KeyView
    // First search scan the prefix of the KeyView against the search key

    let mut i = 0;

    while i < a.len() && i < b.prefix.len() {
        let ord = a[i].cmp(&b.prefix[i]);
        if ord != Ordering::Equal {
            return ord;
        }
        i += 1;
    }

    // If search ended but prefix didn't
    if i == a.len() && i < b.prefix.len() {
        return Ordering::Less;
    }

    // Now we need to continue i with the search key but use j to scan through suffix

    let mut j = 0;

    while i < a.len() && j < b.suffix.len() {
        let ord = a[i].cmp(&b.suffix[j]);
        if ord != Ordering::Equal {
            return ord;
        }
        i += 1;
        j += 1;
    }

    // Length-based tie-break
    match (i == a.len(), j == b.suffix.len()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmp_search() {
        let key_view = KeyView {
            prefix: b"01",
            suffix: b"456",
        };
        let search_key = b"000721";

        assert_eq!(cmp_search(search_key, key_view), Ordering::Less);
    }
}
