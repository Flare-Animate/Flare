//! The number form the string section is written in.

use super::{read_varint, write_varint};

#[test]
fn a_number_reads_back_as_it_was_written() {
    for value in [0, 1, 127, 128, 300, 16_383, 16_384, (1 << 30) - 1, u32::MAX] {
        let mut bytes = Vec::new();
        write_varint(&mut bytes, value);
        assert_eq!(read_varint(&bytes, 0), Some((value, bytes.len())));
    }
}

#[test]
fn a_number_cut_short_is_not_read() {
    assert_eq!(read_varint(&[0x80, 0x80], 0), None);
}
