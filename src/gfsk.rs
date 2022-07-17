use crate::constant::FT8_NN;

pub const FT8_SYMBOL_BT: f32 = 2.0f32;
///< symbol smoothing filter bandwidth factor (BT
const GFSK_CONST_K: f32 = 5.336446f32;
///< == pi * sqrt(2 / log(2))

/// Computes a GFSK smoothing pulse.
/// The pulse is theoretically infinitely long, however, here it's truncated at 3 times the symbol length.
/// This means the pulse array has to have space for 3*n_spsym elements.
/// @param[in] n_spsym Number of samples per symbol
/// @param[in] b Shape parameter (values defined for FT8/FT4)
/// @param[out] pulse Output array of pulse samples
///
pub fn gfsk_pulse(n_spsym: usize, symbol_bt: f32, pulse: &mut Vec<f32>) {
    for i in 0..3 * n_spsym {
        let t = i as f32 / n_spsym as f32 - 1.5;
        let arg1 = GFSK_CONST_K * symbol_bt * (t + 0.5);
        let arg2 = GFSK_CONST_K * symbol_bt * (t - 0.5);
        pulse[i] = (libm::erff(arg1) - libm::erff(arg2)) / 2.0;
    }
}

/// Synthesize waveform data using GFSK phase shaping.
/// The output waveform will contain n_sym symbols.
/// @param[in] symbols Array of symbols (tones) (0-7 for FT8)
/// @param[in] n_sym Number of symbols in the symbol array
/// @param[in] f0 Audio frequency in Hertz for the symbol 0 (base frequency)
/// @param[in] symbol_bt Symbol smoothing filter bandwidth (2 for FT8, 1 for FT4)
/// @param[in] symbol_period Symbol period (duration), seconds
/// @param[in] signal_rate Sample rate of synthesized signal, Hertz
/// @param[out] signal Output array of signal waveform samples (should have space for n_sym*n_spsym samples)
///
pub fn synth_gfsk(
    symbols: &[usize; FT8_NN],
    n_sym: usize,
    f0: f32,
    symbol_bt: f32,
    symbol_period: f32,
    signal_rate: f32,
    signal: &mut Vec<f32>,
) {
    let n_spsym = (0.5 + signal_rate * symbol_period) as usize; // Samples per symbol
    let n_wave = n_sym * n_spsym; // Number of output samples
    let hmod = 1.0f32;

    // Compute the smoothed frequency waveform.
    // Length = (nsym+2)*n_spsym samples, first and last symbols extended
    let dphi_peak = 2.0 * std::f32::consts::PI * hmod / n_spsym as f32;
    let mut dphi = vec![0.0; n_wave + 2 * n_spsym];

    // Shift frequency up by f0
    for i in 0..(n_wave + 2 * n_spsym) {
        dphi[i] = 2.0 * std::f32::consts::PI * f0 / signal_rate;
    }

    let mut pulse = vec![0.0; 3 * n_spsym];

    gfsk_pulse(n_spsym, symbol_bt, &mut pulse);

    for i in 0..n_sym {
        let ib = i * n_spsym;
        for j in 0..3 * n_spsym {
            dphi[j + ib] += dphi_peak * symbols[i] as f32 * pulse[j];
        }
    }

    // Add dummy symbols at beginning and end with tone values equal to 1st and last symbol, respectively
    for j in 0..(2 * n_spsym) {
        dphi[j] += dphi_peak * pulse[j + n_spsym] * symbols[0] as f32;
        dphi[j + n_sym * n_spsym] += dphi_peak * pulse[j] * symbols[n_sym - 1] as f32;
    }

    // Calculate and insert the audio waveform
    let mut phi = 0.0f32;
    for k in 0..n_wave {
        // Don't include dummy symbols
        signal[k] = phi.sin();
        phi = libm::fmodf(phi + dphi[k + n_spsym], 2.0 * std::f32::consts::PI);
    }

    // Apply envelope shaping to the first and last symbols
    let n_ramp = n_spsym / 8;
    for i in 0..n_ramp {
        let env =
            (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (2.0 * n_ramp as f32)).cos()) / 2.0;
        signal[i] *= env;
        signal[n_wave - 1 - i] *= env;
    }
}
