use crate::constant::*;

const TOPBIT: u16 = 1u16 << (FT8_CRC_WIDTH - 1);

pub fn ftx_compute_crc(message: &[u8; FTX_LDPC_K_BYTES], num_bits: usize) -> u16 {
    let mut remainder: u16 = 0;
    let mut idx_byte: usize = 0;

    for idx_bit in 0..num_bits {
        if idx_bit % 8 == 0 {
            remainder ^= (message[idx_byte] as u16) << (FT8_CRC_WIDTH - 8);
            idx_byte += 1;
        }

        if (remainder & TOPBIT) != 0 {
            remainder = (remainder << 1) ^ FT8_CRC_POLYNOMIAL;
        } else {
            remainder = remainder << 1;
        }
    }
    return remainder & ((TOPBIT << 1) - 1u16);
}

pub fn ftx_extract_crc(a91: &[u8; FTX_LDPC_K_BYTES]) -> u16 {
    let chksum: u16 =
        (((a91[9] & 0x07u8) as u16) << 11) | (a91[10] as u16) << 3 | (a91[11] as u16) >> 5;
    return chksum;
}

pub fn ftx_add_crc(payload: &[u8; FTX_LDPC_K_BYTES], a91: &mut [u8; FTX_LDPC_K_BYTES]) {
    // Copy 77 bits of payload data
    for i in 0..10 {
        a91[i] = payload[i];
    }
    // Clear 3 bits after the payload to make 82 bits
    a91[9] &= 0xF8u8;
    a91[10] = 0;

    // Calculate CRC of 82 bits (77 + 5 zeros)
    // 'The CRC is calculated on the source-encoded message, zero-extended from 77 to 82 bits'
    let checksum = ftx_compute_crc(a91, 96 - 14);

    // Store the CRC at the end of 77 bit message
    a91[9] |= (checksum >> 11) as u8;
    a91[10] = (checksum >> 3) as u8;
    a91[11] = (checksum << 5) as u8;
}
