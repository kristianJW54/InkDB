pub(crate) trait OperatorEncoding {
    type Value;

    fn encode(value: Self::Value, buffer: &mut [u8]) -> usize;
    // Maybe a decode
}

// Implementations for different key types
