use libm::log;

use crate::constant::*;
use crate::crc::{ftx_compute_crc, ftx_extract_crc};
use crate::ldpc::*;
use crate::monitor::{Candidate, Waterfall};
use crate::unpack::*;

pub struct FT8FindSync<'a> {
    wf: &'a Waterfall,
}

impl<'a> FT8FindSync<'a> {
    pub fn new(wf: &Waterfall) -> FT8FindSync {
    FT8FindSync { wf }
    }

    fn ft8_sync_score(&self, candidate: &Candidate) -> i32 {
        let mut score = 0i32;
        let mut num_average = 0i32;
        let wf = self.wf;

        for m in 0..FT8_NUM_SYNC {
            for k in 0..FT8_LENGTH_SYNC {
                let block = (FT8_SYNC_OFFSET * m) + k;
                let block_abs = candidate.time_offset + block as i32;

                if block_abs < 0 {
                    continue;
                }
                if block_abs >= wf.num_blocks as i32 {
                    break;
                }
                let p8 = ((block * wf.block_stride) as i32 + wf.get_index(candidate)) as usize;
                let sm = FT8_COSTAS_PATTERN[k] + p8;
                if sm > 0 {
                    score += wf.mag[sm] as i32 - wf.mag[sm - 1] as i32;
                    num_average += 1;
                }
                if sm < 7 {
                    score += wf.mag[sm] as i32 - wf.mag[sm + 1] as i32;
                    num_average += 1;
                }
                if (k > 0) && (block_abs > 0) {
                    score += wf.mag[sm] as i32 - wf.mag[sm - wf.block_stride] as i32;
                    num_average += 1;
                }
                if ((k + 1) < FT8_LENGTH_SYNC) && ((block_abs + 1) < wf.num_blocks as i32) {
                    score += wf.mag[sm] as i32 - wf.mag[sm + wf.block_stride] as i32;
                    num_average += 1;
                }
            }
        }
        if num_average > 0 {
            score /= num_average;
        }
        score
    }

    pub fn ft8_find_sync(
        &mut self,
        time_sub_from: usize,
        time_sub_to: usize,
        min_score: i32,
        candidates: &mut Vec<Candidate>,
    ) -> usize {
        for time_sub in time_sub_from..time_sub_to {
            for freq_sub in 0..self.wf.freq_osr {
                for time_offset in -12..24 {
                    for freq_offset in 0..self.wf.num_bins - 7 {
                        let mut c = Candidate {
                            score: 0,
                            time_offset,
                            freq_offset,
                            time_sub,
                            freq_sub,
                        };
                        let score = self.ft8_sync_score(&c);
                        if score < min_score {
                            continue;
                        }
                        c.score = score;
                        candidates.push(c);
                    }
                }
            }
        }
        candidates.sort_by(|a, b| b.score.cmp(&a.score));
        candidates.len()
    }
}

#[derive(Debug)]
pub struct Message{
    pub df : Vec<(i32, i32, usize)>,
    pub text: String,
    pub hash: u16,
}

impl Message {
    pub fn new() -> Message {
        Message {
            df: Vec::new(),
            text: String::new(),
            hash: 0,
        }
    }
}

pub struct FT8Decode<'a> {
    wf: &'a Waterfall,
    pub message: Vec<Message>,
}

fn max2(a: f32, b: f32) -> f32 {
    if a >= b {
        a
    } else {
        b
    }
}

fn max4(a: f32, b: f32, c: f32, d: f32) -> f32 {
    max2(max2(a, b), max2(c, d))
}

fn pack_bits(bit_array: &[u8; FTX_LDPC_N], num_bits: usize, packed: &mut [u8; FTX_LDPC_K_BYTES]) {
    let num_bytes = (num_bits + 7) / 8;
    for pkd in packed.iter_mut().take(num_bytes) {
        *pkd = 0;
    }

    let mut mask: u8 = 0x80;
    let mut byte_idx: usize = 0;

    for b in bit_array.iter().take(num_bits) {
        if *b != 0 {
            packed[byte_idx] |= mask;
        }
        mask >>= 1;
        if mask == 0 {
            mask = 0x80u8;
            byte_idx += 1;
        }
    }
}

impl<'a> FT8Decode<'a> {
    pub fn new(wf: &'a Waterfall) -> FT8Decode {
        FT8Decode {
            wf,
            message: Vec::new(),
        }
    }

    fn ftx_normalize_logl(&self, log174: &mut [f32; FTX_LDPC_N]) {
        let mut sum = 0.0f32;
        let mut sum2 = 0.0f32;

        for lg in log174.iter().take(FTX_LDPC_N as usize) {
            sum += lg;
            sum2 += lg * lg;
        }

        let inv_n = 1.0f32 / FTX_LDPC_N as f32;
        let variance = (sum2 - (sum * sum * inv_n)) * inv_n;

        let norm_factor = (24.0f32 / variance).sqrt();

        for lg in log174.iter_mut().take(FTX_LDPC_N as usize) {
            *lg *= norm_factor;
        }
    }

    fn ft8_extract_symbol(&self, idx: usize, logl: &mut [f32; FTX_LDPC_N], bit_idx: usize) {
        let mut s2: [f32; 8] = [0.0; 8];

        //3bitグレイコードに対応するトーンの強度をs2に入れる
        for j in 0..8 {
            s2[j] = self.wf.mag[idx + FT8_GRAY_MAP[j]] as f32;
        }
        //グレイコード上のMSBのLLRをtone4-7(1)の最大値からtone0-3(0)の最大値を引いたもの
        logl[bit_idx + 0] = max4(s2[4], s2[5], s2[6], s2[7]) - max4(s2[0], s2[1], s2[2], s2[3]);
        //同様に2bit目はtone2,tone3,tone6,tone7
        //logl[bit_idx + 1] = max4(s2[2], s2[3], s2[6], s2[7]) - max4(s2[0], s2[1], s2[4], s2[5]);
        logl[bit_idx + 1] = max4(s2[2], s2[3], s2[6], s2[7]) - max4(s2[0], s2[1], s2[5], s2[4]);
        //同様に3bitも計算
        //logl[bit_idx + 2] = max4(s2[1], s2[3], s2[5], s2[7]) - max4(s2[0], s2[2], s2[4], s2[6]);
        logl[bit_idx + 2] = max4(s2[1], s2[3], s2[5], s2[7]) - max4(s2[0], s2[2], s2[6], s2[4]);
    
    }

    fn ft8_extract_likelihood(&self, c: &Candidate, log174: &mut [f32; FTX_LDPC_N]) {
        //58bit分のシンボルを取り出す
        for k in 0..FT8_ND {
            //コスタス配列を飛ばしたシンボル部分
            let sym_idx = k + if k < 29 { 7 } else { 14 };
            //3bit分ずつ取り出す
            let bit_idx = 3 * k;

            //ウォーターフォール上でシンボルあるブロック
            let block = c.time_offset + sym_idx as i32;
            //ウォーターフォール外なら0
            if (block < 0) || (block >= self.wf.num_blocks as i32) {
                log174[bit_idx + 0] = 0.0f32;
                log174[bit_idx + 1] = 0.0f32;
                log174[bit_idx + 2] = 0.0f32;
            } else {
                //ウォーターフォール内であればシンボルを展開する
                let idx = (self.wf.get_index(c) + (sym_idx * self.wf.block_stride) as i32) as usize;
                self.ft8_extract_symbol(idx, log174, bit_idx);
            }
        }
    }

    pub fn ft8_decode(&self, c: &Candidate, max_iteration: i32, message: &mut Message) -> bool {
        let mut log174: [f32; FTX_LDPC_N] = [0.0f32; FTX_LDPC_N];

        self.ft8_extract_likelihood(c, &mut log174);
        self.ftx_normalize_logl(&mut log174);

        let mut plain174 = [0u8; FTX_LDPC_N];

        let ldpc_errors = ldpc_decode(log174, max_iteration, &mut plain174);

        if ldpc_errors > 0 {
            return false;
        }

        let mut a91 = [0u8; FTX_LDPC_K_BYTES];

        pack_bits(&plain174, FTX_LDPC_K, &mut a91);

        let crc_extracted = ftx_extract_crc(&a91);
        a91[9] &= 0xf8;
        a91[10] = 0x00;
        let crc_calculated = ftx_compute_crc(&a91, 96 - 14);

        if crc_extracted != crc_calculated {
            /*print!("CRC error! {:?}\n",c);*/
            return false;
        }
        /*
        print!("Decoded message = {:?}\n", plain174);
        print!(
            "Normalized likelihood = {:?}\n",
            log174
                .iter()
                .map(|x| if *x > 0.0 { 1 } else { 0 })
                .collect::<Vec<_>>()
        );
        */
        if unpack77(&a91, &mut message.text) < 0 {
            //print!("Message format error!\n");
            return false;
        }
        message.df.push((c.score, c.time_offset + c.time_sub as i32, c.freq_offset + c.freq_sub));
        message.hash = crc_calculated;
        true
    }
}
