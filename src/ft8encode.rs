use crate::constant::*;
use crate::crc::*;

// Returns 1 if an odd number of bits are set in x, zero otherwise
pub fn parity8(mut x: u8) -> u8 {
    x ^= x >> 4; // a b c d ae bf cg dh
    x ^= x >> 2; // a b ac bd cae dbf aecg bfdh
    x ^= x >> 1; // a ab bac acbd bdcae caedbf aecgbfdh
    x % 2 // modulo 2
}

// Encode via LDPC a 91-bit message and return a 174-bit codeword.
// The generator matrix has dimensions (87,87).
// The code is a (174,91) regular LDPC code with column weight 3.
// Arguments:
// [IN] message   - array of 91 bits stored as 12 bytes (MSB first)
// [OUT] codeword - array of 174 bits stored as 22 bytes (MSB first)
fn encode174(message: &[u8; FTX_LDPC_K_BYTES], codeword: &mut [u8; FTX_LDPC_N_BYTES]) {
    // This implementation accesses the generator bits straight from the packed binary representation in kFTX_LDPC_generator

    // Fill the codeword with message and zeros, as we will only update binary ones later
    for j in 0..FTX_LDPC_N_BYTES {
        codeword[j] = if j < FTX_LDPC_K_BYTES { message[j] } else { 0 };
    }

    // Compute the byte index and bit mask for the first checksum bit
    let mut col_mask = 0x80 >> (FTX_LDPC_K % 8); // bitmask of current byte
    let mut col_idx = FTX_LDPC_K_BYTES - 1; // index into byte array

    // Compute the LDPC checksum bits and store them in codeword
    for i in 0..FTX_LDPC_M {
        // Fast implementation of bitwise multiplication and parity checking
        // Normally nsum would contain the result of dot product between message and kFTX_LDPC_generator[i],
        // but we only compute the sum modulo 2.
        let mut nsum = 0u8;

        for (j, m) in message.iter().enumerate().take(FTX_LDPC_K_BYTES) {
            let bits = m & FTX_LDPC_GENERATOR[i][j]; // bitwise AND (bitwise multiplication)
            nsum ^= parity8(bits); // bitwise XOR (addition modulo 2)
        }

        // Set the current checksum bit in codeword if nsum is odd
        if (nsum % 2) != 0 {
            codeword[col_idx] |= col_mask;
        }

        // Update the byte index and bit mask for the next checksum bit
        col_mask >>= 1;
        if col_mask == 0 {
            col_mask = 0x80;
            col_idx += 1;
        }
    }
}


pub fn ft8_encode(payload: &[u8; FTX_LDPC_K_BYTES], tones: &mut [usize; FT8_NN]) {
    let mut a91 = [0u8; FTX_LDPC_K_BYTES]; // Store 77 bits of payload + 14 bits CRC

    // Compute and add CRC at the end of the message
    // a91 contains 77 bits of payload + 14 bits of CRC
    ftx_add_crc(payload, &mut a91);

    let mut codeword = [0u8; FTX_LDPC_N_BYTES];

    encode174(&a91, &mut codeword);


    // Message structure: S7 D29 S7 D29 S7
    // Total symbols: 79 (FT8_NN)

    let mut mask = 0x80u8; // Mask to extract 1 bit from codeword
    let mut i_byte = 0usize; // Index of the current byte of the codeword

    for i_tone in 0..FT8_NN {
        if i_tone < 7 {
            tones[i_tone] = FT8_COSTAS_PATTERN[i_tone];
        } else if (36..43).contains(&i_tone) {
            tones[i_tone] = FT8_COSTAS_PATTERN[i_tone - 36];
        } else if (72..79).contains(&i_tone) {
            tones[i_tone] = FT8_COSTAS_PATTERN[i_tone - 72];
        } else {
            // Extract 3 bits from codeword at i-th position
            let mut bits3 = 0u8;

            if (codeword[i_byte] & mask) != 0 {
                bits3 |= 4;
            }

            mask >>= 1;
            if mask == 0 {
                mask = 0x80u8;
                i_byte += 1;
            }

            if (codeword[i_byte] & mask) != 0 {
                bits3 |= 2;
            }

            mask >>= 1;
            if mask == 0 {
                mask = 0x80u8;
                i_byte += 1;
            }

            if (codeword[i_byte] & mask) != 0 {
                bits3 |= 1;
            }

            mask >>= 1;
            if mask == 0 {
                mask = 0x80u8;
                i_byte += 1
            }
            //3bitを8つのトーンに変換
            //グレイコードになっているので隣のトーンのビットパターンとのハミング距離(ビットが相違する数)は1になり
            //ドップラー等で周波数が変わってしまった場合でも誤り訂正で修正できる可能性が高い
            tones[i_tone] = FT8_GRAY_MAP[bits3 as usize];
        }
    }
}
#[cfg(test)]
mod tests {
    mod test_utils;
    use crate::ldpc::*;
    use crate::test_utils::*;
    /* 
    let mut codeword_bits = [0u8; FTX_LDPC_N];
    unpack_bits(&codeword, &mut codeword_bits);
    print!("check sum errors = {}\n", ldpc_check(&codeword_bits));
    print!("Raw FT8 codewards = {:?}\n",codeword_bits);
    */
}