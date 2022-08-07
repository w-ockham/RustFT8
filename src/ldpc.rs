use crate::constant::*;

// Sum Productアルゴリズムで使われる
// tanh/atanhを高速化するための近似
fn fast_tanh(x: f32) -> f32 {
    if cfg!(feature = "use_f32tan") {
        x.tanh()
    } else {
        if x < -4.97f32 {
            return -1.0f32;
        }
        if x > 4.97f32 {
            return 1.0f32;
        }
        let x2 = x * x;
        let a = x * (945.0f32 + x2 * (105.0f32 + x2));
        let b = 945.0f32 + x2 * (420.0f32 + x2 * 15.0f32);
        a / b
    }
}

fn fast_atanh(x: f32) -> f32 {
    if cfg!(feature = "use_f32tan") {
        x.atan()
    } else {
        let x2 = x * x;
        let a = x * (945.0f32 + x2 * (-735.0f32 + x2 * 64.0f32));
        let b = 945.0f32 + x2 * (-1050.0f32 + x2 * 225.0f32);
        a / b
    }
}

// codrewordの各ビットがLDPCの検査行列を満たすかチェック
pub fn ldpc_check(codeword: &[u8; FTX_LDPC_N]) -> usize {
    let mut errors: usize = 0;

    //検査行列から1列取り出し
    for m in FTX_LDPC_NM {
        let mut x: u8 = 0;
        for i in m {
            if i != 0 {
                //対応するcodeward中のbitのxorをとる
                x ^= codeword[i - 1];
            }
        }
        //偶数パリティであればこの行はOK
        if x != 0 {
            errors += 1;
        }
    }
    //すべての行を満たす(errors = 0)なら検査行列を満たす
    errors
}

//
// 積和アルゴリズムによるデコーダの実装
//
#[cfg(feature = "ldpc_bp")]
pub fn ldpc_decode(
    codeward: [f32; FTX_LDPC_N],
    max_iters: i32,
    plain: &mut [u8; FTX_LDPC_N],
) -> usize {
    //外部メッセージEを初期化
    let mut tov = [[0.0f32; 7]; FTX_LDPC_N];
    //ビットメッセージを初期化
    let mut toc = [[0.0f32; 7]; FTX_LDPC_M];
    //最小エラー数を取りうる最大値で初期化
    let mut min_errors = FTX_LDPC_M;

    //積和アルゴリズムの繰り返し回数分をループ
    for _it in 0..max_iters {
        let mut plain_sum: u8 = 0;
        //(1) テスト
        for n in 0..FTX_LDPC_N {
            //対数尤度で示されたcodewardの各ビットを外部メッセージEで更新
            //(codewardの1ビットについて3つのチェックノードからの外部メッセージが来る)
            //対数尤度 Log(P(c=1)/P(c=0))で判定しているのでP(c=1)>P(c=0)なら'1'
            //P(c=1)<P(c=0)なら'0'と判定しplain[n]へ格納
            plain[n] = if (codeward[n] + tov[n][0] + tov[n][1] + tov[n][2]) > 0.0f32 {
                1
            } else {
                0
            };
            plain_sum += plain[n];
        }
        //すべてのbitが0の場合は再度繰り返し
        if plain_sum == 0 {
            break;
        }
        //得られたメッセージ列が検査行列を満たすかチェック
        let errors = ldpc_check(plain);
        //パリティエラー数の最小値を更新
        if errors < min_errors {
            min_errors = errors;
            //すべてのビットでエラーがなければデコード完了
            if errors == 0 {
                break;
            }
        }
        //(2)ビットメッセージの更新
        //各検査ノードmに接続するビットノードnからのビットメッセージMを更新する
        for m in 0..FTX_LDPC_M {
            //検査行列の各行の要素を取り出す
            for (n_idx, &n) in FTX_LDPC_NM[m].iter().enumerate() {
                if n != 0 {
                    //検査ノードに接続するビットノードnを求め
                    let n = n - 1;
                    //受信したcodeward[n](ビット位置n)の値を初期値とし
                    let mut tnm = codeward[n];
                    //ビットノードnの外部メッセージE(Extrinsic Message)との和をとる（ただしノードmから来たメッセージは除く）
                    for m_idx in 0..3 {
                        if (FTX_LDPC_MN[n][m_idx] - 1) != m {
                            tnm += tov[n][m_idx];
                        }
                    }
                    //E = - 2 * atan(Π tanh(-M/2))のtanh(-M/2)の部分
                    toc[m][n_idx] = fast_tanh(-tnm / 2.0f32);
                }
            }
        }
        //(3)チェックメッセージの更新
        //各ビットノードnに接続する検査ノードmからの外部メッセージEを更新する
        for n in 0..FTX_LDPC_N {
            for m_idx in 0..3 {
                //ビットノードnに接続する検査ノードmを求め
                let m = FTX_LDPC_MN[n][m_idx] - 1;
                let mut tmn = 1.0f32;
                //検査ノードmのビットメッセージMの積を求める
                for (n_idx, &nn) in FTX_LDPC_NM[m].iter().enumerate() {
                    if (nn != 0) && (nn - 1) != n {
                        tmn *= toc[m][n_idx];
                    }
                }
                // 外部メッセージ E = -2 + atan(Π tanh(-M/2))
                tov[n][m_idx] = -2.0f32 * fast_atanh(tmn);
            }
        }
    }
    min_errors
}

//
//  ビットフリップアルゴリズムによるデコーダの実装
//
#[cfg(feature = "ldpc_bitflip")]
pub fn ldpc_decode(
    codeward: [f32; FTX_LDPC_N],
    max_iters: i32,
    plain: &mut [u8; FTX_LDPC_N],
) -> usize {
    /// 軟判定(log (P(x=1) / P(x=0)))を硬判定(0/1)に変換
    plain.copy_from_slice(&codeward.map(|x| if x >= 0.0 { 1u8 } else { 0 }));

    for _ in 0..max_iters {
        //codeword中の各ビットが各チェックノードの判定で0又は1何れが多いか判定
        let mut votes = vec![vec![0; 2]; FTX_LDPC_N];

        //チェックノードの要素を取り出す
        for e in FTX_LDPC_NM {
            //チェックノードから接続するビットノードbiについてパリティを計算
            for bi in e {
                if bi == 0 {
                    continue;
                }
                let mut x = 0;
                //ビットノードbi以外のビットノードとxorをとる
                for i in e {
                    if i != 0 && i != bi {
                        x ^= plain[i - 1];
                    }
                }
                //チェックサムの結果にもとづきビットノードbiのあるべき値を投票
                //x = 0ならノードbiは0、x = 1ならノードbiは1に一票
                votes[bi - 1][x as usize] += 1;
            }
        }
        // 投票結果にもとづきデコード結果plainの各ビットを更新
        for i in 0..FTX_LDPC_N {
            //対象とするbitが0で投票結果が1の方が多いなら1に反転
            if plain[i] == 0 && (votes[i][1] > votes[i][0]) {
                plain[i] = 1;
            //対象とするbitが1で投票結果が0の方が多いなら0に反転
            } else if plain[i] == 1 && (votes[i][0] > votes[i][1]) {
                plain[i] = 0;
            }
        }
        //　検査行列を満たすかチェック
        if ldpc_check(&plain) == 0 {
            return 0;
        }
    }
    //所定の繰り返しで終わらなければエラー
    return 1;
}
