use realfft::{RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use std::sync::Arc;

pub struct Config {
    pub sample_rate: u32,                         /* Wave sample rate */
    pub symbol_period: f32,                       //< FT4/FT8 symbol period in seconds
    pub slot_time: f32,
    pub time_osr: usize,
    pub freq_osr: usize,
}
#[derive(Debug)]
pub struct  Candidate {
    pub score: i32,
    pub time_offset: i32,
    pub freq_offset: usize,
    pub time_sub: usize,
    pub freq_sub: usize
}

pub struct Waterfall {
    max_blocks: usize, //< number of blocks (symbols) allocated in the mag array
    pub num_blocks: usize, //< number of blocks (symbols) stored in the mag array
    pub num_bins: usize,   //< number of FFT bins in terms of 6.25 Hz
    pub time_osr: usize,     //< number of time subdivisions
    pub freq_osr: usize,     //< number of frequency subdivisions
    pub mag: Vec<u8>,      //< FFT magnitudes stored as uint8_t[blocks][time_osr][freq_osr][num_bins]
    //mag_size: usize,
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
        offset = (offset * self.freq_osr as i32 ) + candidate.freq_sub as i32;
        offset = (offset * self.num_bins as i32) + candidate.freq_offset as i32;
        return offset;       
    }
}

pub struct Monitor<'a> {
    block_size: usize,                        //< Number of samples per symbol (block)
    subblock_size: usize,                     //< Analysis shift size (number of samples)
    nfft: usize,                              //< FFT size
    fft_forward: Arc<dyn RealToComplex<f32>>, // FFT forward
    samples: &'a Vec<f32>,                    // Sampling data
    window: Vec<f32>,                         // Window function
    spectrum: Vec<Complex<f32>>,              // FFT bin
    pub wf: Waterfall,                            //< Waterfall object
    f_min: u32,                               // Minimum Freqency
    f_max: u32,                               // Maximum Frequency
    pub max_mag: f32,                             //< Maximum detected magnitude (debug stats)
}

fn hann(i: usize, n: usize) -> f32 {
    let x = (std::f32::consts::PI * i as f32 / n as f32).sin();
    return x * x;
}

impl<'a> Monitor<'a> {
    pub fn new(config: &Config, samples: &'a Vec<f32>) -> Self {
       
        let block_size = (config.sample_rate as f32 * config.symbol_period) as usize; /* 1920 */
        let subblock_size = block_size / config.time_osr; /* 960 */
        let mut fft = RealFftPlanner::<f32>::new();
        let nfft = block_size * config.freq_osr ; /* 3840 */
        let fft_forward = fft.plan_fft_forward(nfft);
        let fft_norm = 2.0f32 / nfft as f32;
        let max_blocks = (config.slot_time / config.symbol_period) as usize; /* 93 */
        let num_bins = (config.sample_rate as f32 * config.symbol_period / 2.0) as usize; /* 960 */
        let wf = Waterfall::new(max_blocks, num_bins,config.time_osr, config.freq_osr);
        let mut window = Vec::new();
        let mut spectrum = Vec::new();

        for i in 0..nfft {
            window.push(fft_norm * hann(i, nfft));
        }

        for _i in 0..(nfft / 2 + 1) {
            spectrum.push(Complex::new(0.0f32, 0.0f32))
        }

        Monitor {
            block_size,
            subblock_size,
            nfft,
            fft_forward,
            samples,
            window,
            spectrum,
            wf,
            f_min: 100,
            f_max: 3000,
            max_mag: -120.0f32,
        }
    }

    fn process(&mut self, frame: usize) {
        if self.wf.num_blocks >= self.wf.max_blocks {
            return;
        }

        let mut offset = self.wf.num_blocks * self.wf.block_stride;

        for time_sub in 0..self.wf.time_osr {
            let frame_from = frame + time_sub as usize * self.subblock_size;
            let frame_to = frame_from + self.nfft;

            if frame_to > self.samples.len() {
                return;
            }

            // make input and output vectors
            let mut indata = self.samples[frame_from..frame_to].to_vec();

            for (i, v) in indata.iter_mut().enumerate() {
                *v = *v * self.window[i];
            }

            // Forward transform the input data
            self.fft_forward
                .process(&mut indata, &mut self.spectrum)
                .unwrap();

            for freq_sub in 0..self.wf.freq_osr {
                for bin in 0..self.wf.num_bins {
                    let src_bin = bin * self.wf.freq_osr as usize + freq_sub as usize;

                    let mag2 = self.spectrum[src_bin].im * self.spectrum[src_bin].im
                        + self.spectrum[src_bin].re * self.spectrum[src_bin].re;

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
        self.wf.num_blocks += 1;
    }

    pub fn process_all(&mut self) {
        for frame in (0..self.samples.len() - self.block_size).step_by(self.block_size) {
            self.process(frame);
        };
        print!("{} points FFT invoked {} times.\n",
            self.nfft, self.wf.num_blocks  * self.wf.time_osr);
    }
}
