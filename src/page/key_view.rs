use std::cmp::Ordering;

//

pub(crate) struct KeyView<'a> {
    pub(crate) prefix: &'a [u8],
    pub(crate) suffix: &'a [u8],
}

// Comparison logic on here?

impl KeyView<'_> {
    pub(crate) fn can_construct(&self) -> bool {
        if self.prefix.is_empty() {
            return false;
        }
        if self.suffix.is_empty() {
            return false;
        }
        true
    }

    pub(crate) fn cmp_search<'a>(&self, other: &'a [u8]) -> Ordering {
        // Because we are comparing against a search key, we need to use the full key
        other.cmp(&[self.prefix, self.suffix].concat())
    }
}

pub(crate) fn cmp_p2p(a: &KeyView<'_>, b: &KeyView<'_>) -> Ordering {
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
