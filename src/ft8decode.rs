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
    //対象候補の信号とコスタス配列との相関によりスコアを求める
    fn ft8_sync_score(&self, candidate: &Candidate) -> i32 {
        let mut score = 0i32;
        let mut num_average = 0i32;
        let wf = self.wf;

        //FT8に3箇所あるコスタス配列を探す
        for m in 0..FT8_NUM_SYNC {
            //コスタス配列の各要素についてループ
            for k in 0..FT8_LENGTH_SYNC {
                //コスタス配列の開始位置は0,36,72ビット目
                let block = (FT8_SYNC_OFFSET * m) + k;
                let block_abs = candidate.time_offset + block as i32;
                //スペクトログラム内にあることをチェック
                if block_abs < 0 {
                    continue;
                }
                if block_abs >= wf.num_blocks as i32 {
                    break;
                }
                //対象候補のスペクトログラム中の位置を求め
                //コスタス配列との相関をスコア化する
                let p8 = ((block * wf.block_stride) as i32 + wf.get_index(candidate)) as usize;
                let sm = FT8_COSTAS_PATTERN[k];
                //スコアはコスタス配列位置の信号強度とそれ以外の位置のスコアの差分
                //1.コスタス配列内では上下のトーンとの差分をスコアに加算
                if sm > 0 {
                    score += wf.mag[p8 + sm] as i32 - wf.mag[p8 + sm - 1] as i32;
                    num_average += 1;
                }
                if sm < 7 {
                    score += wf.mag[p8 + sm] as i32 - wf.mag[p8 + sm + 1] as i32;
                    num_average += 1;
                }
                //2.前後のシンボルとの差分をスコアに加算
                if (k > 0) && (block_abs > 0) {
                    score += wf.mag[p8 + sm] as i32 - wf.mag[p8 + sm - wf.block_stride] as i32;
                    num_average += 1;
                }
                if ((k + 1) < FT8_LENGTH_SYNC) && ((block_abs + 1) < wf.num_blocks as i32) {
                    score += wf.mag[p8 + sm] as i32 - wf.mag[p8 + sm + wf.block_stride] as i32;
                    num_average += 1;
                }
            }
        }
        //スコアを平均化
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
        //以下の範囲でスペクトログラム上を走査する
        //1.時間・周波数でオーバサンプリングした範囲
        for time_sub in time_sub_from..time_sub_to {
            for freq_sub in 0..self.wf.freq_osr {
                //2. 1.で指定された時間の前後
                for time_offset in -12..24 {
                    //3. STFTで解析した範囲の周波数の範囲(=ビン数)
                    for freq_offset in 0..self.wf.num_bins - 7 {
                        let mut c = Candidate {
                            score: 0,
                            time_offset,
                            freq_offset,
                            time_sub,
                            freq_sub,
                        };
                        //指定された範囲のスコアを求める
                        let score = self.ft8_sync_score(&c);
                        //スコアが所定値以下なら繰り返し
                        if score < min_score {
                            continue;
                        }
                        //デコード候補として格納
                        c.score = score;
                        candidates.push(c);
                    }
                }
            }
        }
        //スコアの高い順にソート
        candidates.sort_by(|a, b| b.score.cmp(&a.score));
        candidates.len()
    }
}

#[derive(Debug)]
pub struct Message {
    pub df: Vec<(i32, f32, f32)>,
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

        //各ビットの分散値から正規化の係数を求め
        for lg in log174.iter() {
            sum += lg;
            sum2 += lg * lg;
        }

        let inv_n = 1.0f32 / FTX_LDPC_N as f32;
        let variance = (sum2 - (sum * sum * inv_n)) * inv_n;

        //正規化係数を各ビットにかけて正規化
        let norm_factor = (24.0f32 / variance).sqrt();
        for lg in log174.iter_mut() {
            *lg *= norm_factor;
        }
    }

    fn ft8_extract_symbol(&self, idx: usize, logl: &mut [f32; FTX_LDPC_N], bit_idx: usize) {
        let mut s2: [f32; 8] = [0.0; 8];
        //3bitグレイコードに対応するトーンの強度をs2に入れる
        for j in 0..8 {
            s2[j] = self.wf.mag[idx + FT8_GRAY_MAP[j]] as f32;
        }
        //各bit毎の対数尤度LLR(Log Likelihood Ratio)を個別に求める　LLR = log(P(b=1)/P(b=0))
        //グレイコード上のMSBのLLRはtone4-7(1)の最大値からtone0-3(0)の最大値を引いたもの
        logl[bit_idx + 0] = max4(s2[4], s2[5], s2[6], s2[7]) - max4(s2[0], s2[1], s2[2], s2[3]);
        //同様に2bit目はtone2,3,6,7(1)の最大値からtone0,1,5,4(0)の最大値を引いたもの
        logl[bit_idx + 1] = max4(s2[2], s2[3], s2[6], s2[7]) - max4(s2[0], s2[1], s2[5], s2[4]);
        //同様に3bitも計算
        logl[bit_idx + 2] = max4(s2[1], s2[3], s2[5], s2[7]) - max4(s2[0], s2[2], s2[6], s2[4]);
    }

    fn ft8_extract_likelihood(&self, c: &Candidate, log174: &mut [f32; FTX_LDPC_N]) {
        //58bit分のシンボルを取り出す
        for k in 0..FT8_ND {
            //コスタス配列を飛ばしたシンボル部分
            let sym_idx = k + if k < 29 { 7 } else { 14 };
            //3bit分ずつ取り出す
            let bit_idx = 3 * k;

            //スペクトログラム上でシンボルがあるブロックを取り出す
            let block = c.time_offset + sym_idx as i32;
            //スペクトログラム外なら0
            if (block < 0) || (block >= self.wf.num_blocks as i32) {
                log174[bit_idx + 0] = 0.0f32;
                log174[bit_idx + 1] = 0.0f32;
                log174[bit_idx + 2] = 0.0f32;
            } else {
                //スペクトログラム内であればシンボルを対数尤度で取り出す
                let idx = (self.wf.get_index(c) + (sym_idx * self.wf.block_stride) as i32) as usize;
                self.ft8_extract_symbol(idx, log174, bit_idx);
            }
        }
    }

    pub fn ft8_decode(&self, c: &Candidate, max_iteration: i32, message: &mut Message) -> bool {
        let mut log174: [f32; FTX_LDPC_N] = [0.0f32; FTX_LDPC_N];

        //デコード候補のある位置のスペクトログラムからシンボルを取り出す
        self.ft8_extract_likelihood(c, &mut log174);
        //各ビットのLLRを正規化
        self.ftx_normalize_logl(&mut log174);

        let mut plain174 = [0u8; FTX_LDPC_N];
        // LDPCデコードを実行
        let ldpc_errors = ldpc_decode(log174, max_iteration, &mut plain174);

        if ldpc_errors > 0 {
            return false;
        }

        let mut a91 = [0u8; FTX_LDPC_K_BYTES];

        //ビット列plain174をa91にパック
        pack_bits(&plain174, FTX_LDPC_K, &mut a91);
        //受信時に得られたCRCを取り出す
        let crc_extracted = ftx_extract_crc(&a91);
        //CRC部分をマスク
        a91[9] &= 0xf8;
        a91[10] = 0x00;
        //再度メッセージからCRCを計算する
        let crc_calculated = ftx_compute_crc(&a91, 96 - 14);

        //受信時のCRCと受信メッセージから生成したCRCが異なればデコード失敗
        if crc_extracted != crc_calculated {
            return false;
        }

        //パックされたビット列からメッセージを展開
        if unpack77(&a91, &mut message.text) < 0 {
            return false;
        }
        
        //メッセージのDF/DTを求め
        let freq_hz = (c.freq_offset as f32 + c.freq_sub as f32 / self.wf.freq_osr as f32)
            / FT8_SYMBOL_PERIOD;
        let time_sec = (c.time_offset as f32 + c.time_sub as f32 / self.wf.time_osr as f32)
            * FT8_SYMBOL_PERIOD;

        //メッセージのCRCをキーにデコードされたメッセージをハッシュに登録
        message.hash = crc_calculated;
        message.df.push((c.score, time_sec, freq_hz));
        true
    }
}
