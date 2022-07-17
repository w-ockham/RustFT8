use crate::constant::*;

fn fast_tanh(x: f32) -> f32 {
    if x < -4.97f32 {
        return -1.0f32;
    }
    if x > 4.97f32 {
        return 1.0f32;
    }
    let x2 = x * x;
    let a = x * (945.0f32 + x2 * (105.0f32 + x2));
    let b = 945.0f32 + x2 * (420.0f32 + x2 * 15.0f32);
    return a / b;
}

fn fast_atanh(x: f32) -> f32 {
    let x2 = x * x;
    let a = x * (945.0f32 + x2 * (-735.0f32 + x2 * 64.0f32));
    let b = 945.0f32 + x2 * (-1050.0f32 + x2 * 225.0f32);
    return a / b;
}

fn ldpc_check(codeward: &[u8; FTX_LDPC_N]) -> usize {
    let mut errors: usize = 0;

    for m in FTX_LDPC_NM {
        let mut x: u8 = 0;
        for i in m {
            if i != 0 {
                x ^= codeward[i - 1];
            }
        }
        if x != 0 {
            errors += 1;
        }
    }
    return errors;
    /*
    for m in 0..FTX_LDPC_M {
        let mut x: u8 = 0;
        for i in 0..FTX_LDPC_NUM_ROWS[m] {
            x ^= codeward[FTX_LDPC_NM[m][i] - 1];
        }
        if x != 0 {
            errors += 1;
        }
    }
    return errors;
    */
}

/*
pub fn ldpc_decode(codeward:&[f32;FTX_LDPC_N], max_iters: i32, plain: &mut[u8;FTX_LDPC_N]) -> usize {
    let mut m : [[f32; FTX_LDPC_N]; FTX_LDPC_M] = [[0.0; FTX_LDPC_N]; FTX_LDPC_M];
    let mut e : [[f32; FTX_LDPC_N]; FTX_LDPC_M] = [[0.0; FTX_LDPC_N]; FTX_LDPC_M];
    let mut min_errors = FTX_LDPC_M;

    for j in 0..FTX_LDPC_M {
        for i in 0..FTX_LDPC_N {
            m[j][i] = codeward[i];
        }
    }

    for _it in 0..max_iters {
        for j in 0..FTX_LDPC_M {
            for ii1 in 0..FTX_LDPC_NUM_ROWS[j] {
                let i1 = FTX_LDPC_NM[j][ii1] - 1;
                let mut a = 1.0f32;
                for ii2 in 0.. FTX_LDPC_NUM_ROWS[j] {
                    let i2 = FTX_LDPC_NM[j][ii2] - 1;
                    if i2 != i1 {
                        a *= fast_atanh(a)
                    }
                }
                e[j][i1] = -2.0f32 * fast_atanh(a);
            }
        }

        for i in 0..FTX_LDPC_N {
            let mut l = codeward[i];
            for j in 0..3 {
                l += e[FTX_LDPC_MN[i][j] -1][i];
            }
            plain[i] = if l > 0.0f32 { 1 } else { 0 };
        }

        let errors = ldpc_check(plain);

        if errors < min_errors {
            min_errors = errors;

            if errors == 0 {
                break;
            }
        }

        for i in 0.. FTX_LDPC_N {
            for ji1 in 0..3 {
                let j1 = FTX_LDPC_MN[i][ji1] - 1;
                let mut l = codeward[i];
                for ji2 in 0..3 {
                    if ji1 != ji2 {
                        let j2 = FTX_LDPC_MN[i][ji2] - 1;
                        l += e[j2][i];
                    }
                }
                m[j1][i] = l;
            }
        }
    }

    return min_errors;
}
*/

pub fn bp_decode(
    codeward: [f32; FTX_LDPC_N],
    max_iters: i32,
    plain: &mut [u8; FTX_LDPC_N],
) -> usize {
    let mut tov: [[f32; 3]; FTX_LDPC_N] = [[0.0f32; 3]; FTX_LDPC_N];
    let mut toc: [[f32; 7]; FTX_LDPC_M] = [[0.0f32; 7]; FTX_LDPC_M];

    let mut min_errors = FTX_LDPC_M;

    for _it in 0..max_iters {
        let mut plain_sum: u8 = 0;
        for n in 0..FTX_LDPC_N {
            plain[n] = if (codeward[n] + tov[n][0] + tov[n][1] + tov[n][2]) > 0.0f32 {
                1
            } else {
                0
            };
            plain_sum += plain[n];
        }

        if plain_sum == 0 {
            break;
        }

        let errors = ldpc_check(plain);

        if errors < min_errors {
            min_errors = errors;
            if errors == 0 {
                break;
            }
        }

        for m in 0..FTX_LDPC_M {
            for n_idx in 0..FTX_LDPC_NUM_ROWS[m] {
                let n = FTX_LDPC_NM[m][n_idx] - 1;
                let mut tnm = codeward[n];

                for m_idx in 0..3 {
                    if (FTX_LDPC_MN[n][m_idx] - 1) != m {
                        tnm += tov[n][m_idx];
                    }
                }
                toc[m][n_idx] = fast_tanh(-tnm / 2.0f32);
            }
        }

        for n in 0..FTX_LDPC_N {
            for m_idx in 0..3 {
                let m = FTX_LDPC_MN[n][m_idx] - 1;
                let mut tmn = 1.0f32;

                for n_idx in 0..FTX_LDPC_NUM_ROWS[m] {
                    if (FTX_LDPC_NM[m][n_idx] - 1) != n {
                        tmn *= toc[m][n_idx];
                    }
                }
                tov[n][m_idx] = -2.0f32 * fast_atanh(tmn);
            }
        }
    }

    return min_errors;
}
