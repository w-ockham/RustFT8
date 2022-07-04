
use crate::constant::{*};

const TOPBIT: u16 = 1u16 << (FT8_CRC_WIDTH -1 );

pub fn ftx_compute_crc(message: &[u8; FTX_LDPC_K_BYTES], num_bits: usize) -> u16 {
    let mut reminder: u16 = 0;
    let mut idx_byte: usize = 0;

    for idx_bit in 0..num_bits {
        if idx_bit %8 == 0 {
            reminder ^= (message[idx_byte] as u16) << (FT8_CRC_WIDTH - 8);
            idx_byte += 1;
        }
    }

    return reminder & ((TOPBIT << 1) - 1u16);
}

pub fn ftx_extract_crc(a91: &[u8; FTX_LDPC_K_BYTES]) -> u16 {
    let chksum: u16 = (((a91[9] & 0x07u8) as u16) << 11) | (a91[10] as u16) << 3 | (a91[11] as u16) >> 5;
    return chksum;
}
