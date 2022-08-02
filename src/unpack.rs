use crate::constant::*;
use crate::text::*;

const MAX22: u32 = 4194304;
const NTOKENS: u32 = 2063592;
const MAXGRID4: u16 = 32400;

// n28 is a 28-bit integer, e.g. n28a or n28b, containing all the
// call sign bits from a packed message.
pub fn unpack_callsign(n28: u32, ip: u8, i3: u8, result: &mut String) -> bool {
    // Check for special tokens DE, QRZ, CQ, CQ_nnn, CQ_aaaa
    if n28 < NTOKENS {
        if n28 <= 2 {
            if n28 == 0 {
                result.push_str("DE");
            }
            if n28 == 1 {
                result.push_str("QRZ");
            }
            if n28 == 2 {
                result.push_str("CQ");
            }
            return false;
        }
        if n28 <= 1002 {
            // CQ_nnn with 3 digits
            result.push_str("CQ ");
            int_to_dd(result, n28 as i32 - 3, 3, false);
            return false; // Success
        }
        if n28 <= 532443 {
            // CQ_aaaa with 4 alphanumeric symbols
            let mut n = n28 - 1003;
            let mut aaaa = String::new();

            for _i in (0..4).rev() {
                aaaa.push(charn((n % 27) as u8, 4));
                n /= 27;
            }

            result.push_str("CQ ");
            result.push_str(aaaa.chars().rev().collect::<String>().trim());
            return false; // Success
        }
        // ? TODO: unspecified in the WSJT-X code
        return true;
    }

    let n28 = n28 - NTOKENS;
    if n28 < MAX22 {
        // This is a 22-bit hash of a result
        // TODO: implement
        result.push_str("<...>");
        // result[0] = '<';
        // int_to_dd(result + 1, n28, 7, false);
        // result[8] = '>';
        // result[9] = '\0';
        return false;
    }

    // Standard callsign
    let mut n = n28 - MAX22;

    let mut callsign = String::new();

    callsign.push(charn((n % 27) as u8, 4));
    n /= 27;
    callsign.push(charn((n % 27) as u8, 4));
    n /= 27;
    callsign.push(charn((n % 27) as u8, 4));
    n /= 27;
    callsign.push(charn((n % 10) as u8, 3));
    n /= 10;
    callsign.push(charn((n % 36) as u8, 2));
    n /= 36;
    callsign.push(charn((n % 37) as u8, 1));

    // Skip trailing and leading whitespace in case of a short callsign
    result.push_str(callsign.chars().rev().collect::<String>().trim());

    if result.is_empty() {
        return true;
    }

    // Check if we should append /R or /P suffix
    if ip != 0 {
        if i3 == 1 {
            result.push_str("/R");
        } else if i3 == 2 {
            result.push_str("/P");
        }
    }

    false
}

pub fn unpack_type1(
    a77: &[u8; FTX_LDPC_K_BYTES],
    i3: u8,
    call_to: &mut String,
    call_de: &mut String,
    extra: &mut String,
) -> i32 {
    // Extract packed fields
    let mut n28a = (a77[0] as u32) << 21;
    n28a |= (a77[1] as u32) << 13;
    n28a |= (a77[2] as u32) << 5;
    n28a |= (a77[3] as u32) >> 3;

    let mut n28b = ((a77[3] & 0x07) as u32) << 26;
    n28b |= (a77[4] as u32) << 18;
    n28b |= (a77[5] as u32) << 10;
    n28b |= (a77[6] as u32) << 2;
    n28b |= (a77[7] as u32) >> 6;

    let ir = (a77[7] & 0x20) as u16 >> 5;
    let mut igrid4 = ((a77[7] & 0x1F) as u16) << 10;
    igrid4 |= (a77[8] as u16) << 2;
    igrid4 |= (a77[9] as u16) >> 6;

    // Unpack both callsigns
    if unpack_callsign(n28a >> 1, n28a as u8 & 0x01, i3, call_to) {
        return -1;
    }

    if unpack_callsign(n28b >> 1, n28b as u8 & 0x01, i3, call_de) {
        return -2;
    }

    if igrid4 <= MAXGRID4 {
        // Extract 4 symbol grid locator
        if ir > 0 {
            // In case of ir=1 add an "R" before grid
            extra.push_str("R ");
        }

        let mut n = igrid4;
        let mut dst = String::new();

        dst.push((b'0' + (n % 10) as u8) as char);
        n /= 10;
        dst.push((b'0' + (n % 10) as u8) as char);
        n /= 10;
        dst.push((b'A' + (n % 18) as u8) as char);
        n /= 18;
        dst.push((b'A' + (n % 18) as u8) as char);

        extra.push_str(dst.chars().rev().collect::<String>().trim());
    } else {
        // Extract report
        let irpt = igrid4 - MAXGRID4;

        // Check special cases first (irpt > 0 always)
        match irpt {
            1 => extra.push_str(""),
            2 => extra.push_str("RRR"),
            3 => extra.push_str("RR73"),
            4 => extra.push_str("73"),
            _ => {
                // Extract signal report as a two digit number with a + or - sign
                if ir > 0 {
                    extra.push('R')
                }
                int_to_dd(extra, irpt as i32 - 35, 2, true);
            }
        }
    }
    0 // Success
}

pub fn unpack_text(a71: &[u8; FTX_LDPC_K_BYTES], text: &mut String) -> i32 {
    // TODO: test
    let mut b71 = [0u8; 9];

    // Shift 71 bits right by 1 bit, so that it's right-aligned in the byte array
    let mut carry = 0;
    for i in 0..9 {
        b71[i] = carry | (a71[i] >> 1);
        carry = if (a71[i] & 1) != 0 { 0x80 } else { 0 };
    }

    let mut c14 = String::new();

    for _idx in 0..13 {
        // Divide the long integer in b71 by 42
        let mut rem = 0u16;
        for b in &mut b71 {
            rem = (rem << 8) | (*b as u16);
            *b = (rem / 42) as u8;
            rem %= 42;
        }
        c14.push(charn(rem as u8, 0));
    }

    text.push_str(c14.chars().rev().collect::<String>().trim());
    0 // Success
}

pub fn unpack_telemetry(a71: &[u8; FTX_LDPC_K_BYTES], telemetry: &mut String) -> i32 {
    let mut b71 = [0u8; 9];

    // Shift bits in a71 right by 1 bit
    let mut carry = 0u8;
    for i in 0..9 {
        b71[i] = (carry << 7) | (a71[i] >> 1);
        carry = a71[i] & 0x01;
    }

    // Convert b71 to hexadecimal string
    for b in &b71 {
        let nibble1 = *b >> 4;
        let nibble2 = *b & 0x0F;
        let c1 = if nibble1 > 9 {
            (nibble1 - 10 + b'A') as char
        } else {
            (nibble1 + b'0') as char
        };
        let c2 = if nibble2 > 9 {
            (nibble2 - 10 + b'A') as char
        } else {
            (nibble2 + b'0') as char
        };
        telemetry.push(c1);
        telemetry.push(c2);
    }

    0
}

//none standard for wsjt-x 2.0
//by KD8CEC
pub fn unpack_nonstandard(
    a77: &[u8; FTX_LDPC_K_BYTES],
    call_to: &mut String,
    call_de: &mut String,
    extra: &mut String,
) -> i32 {
    
    //let mut n12 = (a77[0] << 4) as u32; //11 ~4  : 8
    //n12 |= (a77[1] as u32) >> 4; //3~0 : 12

    let mut n58 = ((a77[1] & 0x0F) as u64) << 54; //57 ~ 54 : 4
    n58 |= (a77[2] as u64) << 46; //53 ~ 46 : 12
    n58 |= (a77[3] as u64) << 38; //45 ~ 38 : 12
    n58 |= (a77[4] as u64) << 30; //37 ~ 30 : 12
    n58 |= (a77[5] as u64) << 22; //29 ~ 22 : 12
    n58 |= (a77[6] as u64) << 14; //21 ~ 14 : 12
    n58 |= (a77[7] as u64) << 6; //13 ~ 6 : 12
    n58 |= (a77[8] as u64) >> 2; //5 ~ 0 : 765432 10

    let iflip = ((a77[8] as u32) >> 1) & 0x01; //76543210
    let mut nrpt = ((a77[8] as u32) & 0x01) << 1;
    nrpt |= (a77[9] as u32) >> 7; //76543210

    let icq = ((a77[9] as u32) >> 6) & 0x01;

    let mut c11 = String::new();

    for _i in (0..11).rev() {
        c11.push(charn((n58 % 38) as u8, 5));
        n58 /= 38;
    }

    let mut call_3 = String::new();
    // should replace with hash12(n12, call_3);
    call_3.push_str("<...>");
    // call_3[0] = '<';
    // int_to_dd(call_3 + 1, n12, 4, false);
    // call_3[5] = '>';
    // call_3[6] = '\0';
    let c11r = c11.chars().rev().collect::<String>();
    let (call_1, call_2) = if iflip != 0 {
        (c11r, call_3)
    } else {
        (call_3, c11r)
    };
    //save_hash_call(c11_trimmed);

    if icq == 0 {
        call_to.push_str(call_1.as_str());
        if nrpt == 1 {
            extra.push_str("RRR");
        } else if nrpt == 2 {
            extra.push_str("RR73");
        } else if nrpt == 3 {
            extra.push_str("73");
        }
    } else {
        call_to.push_str("CQ");
    }

    call_de.push_str(call_2.as_str());

    0
}

pub fn unpack77_fields(
    a77: &[u8; FTX_LDPC_K_BYTES],
    call_to: &mut String,
    call_de: &mut String,
    extra: &mut String,
) -> i32 {
    // Extract i3 (bits 74..76)
    let i3 = (a77[9] >> 3) & 0x07;

    if i3 == 0 {
        // Extract n3 (bits 71..73)
        let n3 = ((a77[8] << 2) & 0x04) | ((a77[9] >> 6) & 0x03);

        if n3 == 0 {
            // 0.0  Free text
            return unpack_text(a77, extra);
        } else if n3 == 5 {
            return unpack_telemetry(a77, extra);
        }
    } else if i3 == 1 || i3 == 2 {
        // Type 1 (standard message) or Type 2 ("/P" form for EU VHF contest)
        return unpack_type1(a77, i3, call_to, call_de, extra);
    } else if i3 == 4 {
        //     // Type 4: Nonstandard calls, e.g. <WA9XYZ> PJ4/KA1ABC RR73
        //     // One hashed call or "CQ"; one compound or nonstandard call with up
        //     // to 11 characters; and (if not "CQ") an optional RRR, RR73, or 73.
        return unpack_nonstandard(a77, call_to, call_de, extra);
    }
    -1
}

pub fn unpack77(a77: &[u8; FTX_LDPC_K_BYTES], message: &mut String) -> i32 {
    let mut call_to = String::new();
    let mut call_de = String::new();
    let mut extra = String::new();

    let rc = unpack77_fields(a77, &mut call_to, &mut call_de, &mut extra);
    if rc < 0 {
        return rc;
    }

    // int msg_sz = strlen(call_to) + strlen(call_de) + strlen(extra) + 2;
    if !call_to.is_empty() {
        message.push_str(&call_to);
        message.push(' ');
    }
    if !call_de.is_empty() {
        message.push_str(&call_de);
        message.push(' ');
    }
    if !extra.is_empty() {
        message.push_str(&extra);
    }

    0
}
