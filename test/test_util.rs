
pub fn unpack_bits(codeward: &[u8; FTX_LDPC_N_BYTES], codeword_bits: &mut [u8; FTX_LDPC_N]) {
    let mut idx = 0;
    for c in codeward.iter() {
        let mut c = *c;
        for _ in 0..8 {
            codeword_bits[idx] = if (0x80 & c) == 0 { 0 } else { 1 };
            c = c << 1;
            idx += 1;
            if idx >= 174 {
                break;
            }
        }
    }
}
