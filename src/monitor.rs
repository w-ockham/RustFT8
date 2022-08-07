use crate::constant::{FT8_SYMBOL_PERIOD, FT8_SLOT_TIME};
use crate::spectrogram::*;
use plotters::prelude::*;
use realfft::{RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use std::sync::Arc;

pub struct Config {
    pub sample_rate: u32,   /* Wave sample rate */
    pub time_osr: usize,
    pub freq_osr: usize,
    pub sync_min_score: i32,
    pub num_threads: usize,
    pub ldpc_max_iteration: i32,
}
#[derive(Debug)]
pub struct Candidate {
    pub score: i32,
    pub time_offset: i32,
    pub freq_offset: usize,
    pub time_sub: usize,
    pub freq_sub: usize,
}

pub struct Waterfall {
    pub max_blocks: usize, // number of blocks (symbols) allocated in the mag array
    pub num_blocks: usize, // number of blocks (symbols) stored in the mag array
    pub num_bins: usize,   // number of FFT bins in terms of 6.25 Hz
    pub time_osr: usize,   // number of time subdivisions
    pub freq_osr: usize,   // number of frequency subdivisions
    pub mag: Vec<u8>, //<FFT magnitudes stored as uint8_t[blocks][time_osr][freq_osr][num_bins]
    pub block_stride: usize, //< Helper value = time_osr * freq_osr * num_bins
}

impl Waterfall {
    pub fn new(max_blocks: usize, num_bins: usize, time_osr: usize, freq_osr: usize) -> Self {
        let mag_size = max_blocks * time_osr * freq_osr * num_bins;
        let block_stride = time_osr * freq_osr * num_bins;
        let mut mag = Vec::with_capacity(mag_size);
        for _i in 0..mag_size {
            mag.push(0);
        }
        Waterfall {
            max_blocks,
            num_blocks: 0,
            num_bins,
            time_osr,
            freq_osr,
            mag,
            block_stride,
        }
    }

    pub fn get_index(&self, candidate: &Candidate) -> i32 {
        let mut offset = candidate.time_offset;
        offset = (offset * self.time_osr as i32) + candidate.time_sub as i32;
        offset = (offset * self.freq_osr as i32) + candidate.freq_sub as i32;
        offset = (offset * self.num_bins as i32) + candidate.freq_offset as i32;
        offset
    }
}

pub struct Monitor<'a> {
    pub block_size: usize, // Number of samples per symbol (block)
    subblock_size: usize,  // Analysis shift size (number of samples)
    nfft: usize,           // FFT size
    fft_forward: Arc<dyn RealToComplex<f32>>, // FFT forward
    samples: &'a Vec<f32>, // Sampling data
    window: Vec<f32>,      // Window function
    spectrum: Vec<Complex<f32>>, // FFT bin
    pub wf: Waterfall,     // Waterfall object
    pub max_mag: f32,      // Maximum detected magnitude (debug stats)
}

// FFT窓関数：矩形窓
#[cfg(feature = "window_rect")]
fn wfunc(i: usize, n: usize) -> f32 {
    1.0
}

// FFT窓関数：ハン窓
#[cfg(feature = "window_hann")]
fn wfunc(i: usize, n: usize) -> f32 {
    let x = (std::f32::consts::PI * i as f32 / n as f32).sin().powi(2);
    x
}

// FFT窓関数：ハミング窓
#[cfg(feature = "window_hamming")]
fn wfunc(i: usize, n: usize) -> f32 {
    let a0 = 0.54;
    let a1 = 0.46;
    let pi2 = 2.0 * std::f32::consts::PI;
    let x = a0 - a1 *(pi2 * i as f32 / n as f32).cos();
    x
}

// FFT窓関数：ブラックマン窓
#[cfg(feature = "window_blackman")]
fn wfunc(i: usize, n: usize) -> f32 {
    let a0 = 0.42;
    let a1 = 0.5;
    let a2 = 0.08;
    let pi2 = 2.0 * std::f32::consts::PI;
    let x = i as f32 / n as f32;
    let w = a0 - a1 *(pi2 * x).cos() + a2 * (2.0 * pi2 * x).cos();
    w
}

impl<'a> Monitor<'a> {
    pub fn new(config: &Config, samples: &'a Vec<f32>) -> Self {
        let block_size = (config.sample_rate as f32 * FT8_SYMBOL_PERIOD) as usize; /* 1920 */
        let subblock_size = block_size / config.time_osr; /* 960 */
        let mut fft = RealFftPlanner::<f32>::new();
        let nfft = block_size * config.freq_osr; /* 3840 */
        let fft_forward = fft.plan_fft_forward(nfft);
        let fft_norm = 2.0f32 / nfft as f32;
        let max_blocks = (FT8_SLOT_TIME / FT8_SYMBOL_PERIOD) as usize; /* 93 */
        let num_bins = (config.sample_rate as f32 * FT8_SYMBOL_PERIOD / 2.0) as usize; /* 960 */
        let wf = Waterfall::new(max_blocks, num_bins, config.time_osr, config.freq_osr);
        let mut window = Vec::new();
        let mut spectrum = Vec::new();

        for i in 0..nfft {
            window.push(fft_norm * wfunc(i, nfft));
        }

        for _i in 0..(nfft / 2 + 1) {
            spectrum.push(Complex::new(0.0f32, 0.0f32))
        }
        println!(
            "block size ={}, subblock_size = {}, num of fft = {}, max_block = {}, num of bin = {}",
            block_size, subblock_size, nfft, max_blocks, num_bins
        );
        Monitor {
            block_size,
            subblock_size,
            nfft,
            fft_forward,
            samples,
            window,
            spectrum,
            wf,
            max_mag: -120.0f32,
        }
    }

    fn process(&mut self, frame: usize) {
        if self.wf.num_blocks >= self.wf.max_blocks {
            return;
        }
        //現在のブロックのバッファのオフセット値を求める
        let mut offset = self.wf.num_blocks * self.wf.block_stride;

        //時間方向のオーバーサンプル単位でサンプル列を切り出す
        for time_sub in 0..self.wf.time_osr {
            //STFTの対象となるサンプル列のはじまりと終わりを求める
            //ここでsubblock_sizeはシンボルピリオドを時間方向のオーバサンプルで割ったサイズ
            let frame_from = frame + time_sub as usize * self.subblock_size;
            let frame_to = frame_from + self.nfft;

            if frame_to > self.samples.len() {
                return;
            }
            //サンプル列からFFTの対象となるnfft点分を部分を取り出し窓関数をかける
            //周波数方向も2倍オーバサンプルしているのでnfft=3840
            let mut indata = self.samples[frame_from..frame_to].to_vec();
            for (i, v) in indata.iter_mut().enumerate() {
                *v *= self.window[i];
            }

            // 実数FFTを実行
            self.fft_forward
                .process(&mut indata, &mut self.spectrum)
                .unwrap();
            // FFTの結果は outputに複素数として得られる。サイズはエイリアス分を除いた nfft / 2 + 1個
            // ここで各bin[n]の周波数 f(n) = (fs / nfft) * n = (12000 / 3480) * n = 3.125 * n (Hz)
            // 周波数方向のオーバーサンプル単位にパワースペクトラムを求める
            for freq_sub in 0..self.wf.freq_osr {
                for bin in 0..self.wf.num_bins {
                    //一つおきにbinを求めるので6.25Hz単位
                    let src_bin = bin * self.wf.freq_osr as usize + freq_sub as usize;
                    //binのパワーを求め
                    let mag2 = self.spectrum[src_bin].im * self.spectrum[src_bin].im
                        + self.spectrum[src_bin].re * self.spectrum[src_bin].re;
                    //デシベルに変換し8bitにスケーリングする
                    let db = 10.0 * (1e-12 + mag2).log10();
                    let scaled = (2.0 * db + 240.0) as i32;

                    let mag = if scaled < 0 {
                        0
                    } else if scaled > 255 {
                        255
                    } else {
                        scaled as u8
                    };
                    self.wf.mag[offset] = mag;
                    offset += 1;
                    if db > self.max_mag {
                        self.max_mag = db;
                    }
                }
            }
        }
        //次のブロックへ
        self.wf.num_blocks += 1;
    }

    pub fn process_all(&mut self) {
        //シンボルピリオド(0.16s = 1920 blotck size)毎にShort Time FFTを実行
        for frame in (0..self.samples.len() - self.block_size).step_by(self.block_size) {
            self.process(frame);
        }
        println!(
            "{} points FFT invoked {} times.",
            self.nfft,
            self.wf.num_blocks * self.wf.time_osr
        );
    }

    // スペクトログラムをファイルにダンプ
    pub fn dump_spectrogram(&self, path: &str) {
        let x_axis = self.nfft / 2;
        let y_axis = self.wf.max_blocks * self.wf.time_osr;
        let mut spectr = Vec::new();

        for y in 0..y_axis {
            for x in 0..x_axis / 2 {
                spectr.push(self.wf.mag[x + y * x_axis]);
                //オーバーサンプルした分を元に戻す(freq_osr = 2の時のみ対応)
                spectr.push(self.wf.mag[x + x_axis / 2 + y * x_axis]);
            }
        }
        let root = BitMapBackend::new(path, (x_axis as u32, y_axis as u32)).into_drawing_area();
        plot_spectrogram(&spectr, x_axis, y_axis, &root);
    }
}
