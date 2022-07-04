mod constant;
mod monitor;
mod ft8decode;
mod ldpc;
mod crc;

use std::sync::Arc;
use std::fs::File;
use std::time::Instant;
use wav_io::header::WavHeader;
use monitor::{Monitor, Config};
use ft8decode::{*};
use ldpc::{*};
use crc::{*};

fn main() {
let input_wav = File::open("data/191111_110130.wav").unwrap();
    let (header, samples) = wav_io::read_from_file(input_wav).unwrap();
    println!("{:?} {}", header, header.sample_rate);

    let start = Instant::now();
    
    let config = Config {
        sample_rate : header.sample_rate,
        symbol_period: 0.16f32,
        slot_time : 15.0f32,
        time_osr : 2,
        freq_osr : 2,
    };
    let mut mon = Monitor::new(&config, &samples);
    mon.process_all();
    print!("Elapsed = {:?} Max mag = {}\n",start.elapsed(), mon.max_mag);

    let mut find_sync:FT8FindSync = FT8FindSync::new(&mon.wf);
    let num = find_sync.ft8_find_sync(10);
    print!("Elapsed = {:?} Candidates = {} \n",start.elapsed(), num);

    let mut decode = FT8Decode::new(&mon.wf);

    for c in find_sync.candidates.iter() {
        if decode.ft8_decode(c, 80) {
            let freq_hz = (c.freq_offset as f32 + c.freq_sub as f32 / mon.wf.freq_osr as f32) / config.symbol_period as f32;
            let time_sec = (c.time_offset as f32 + c.time_sub as f32 / mon.wf.time_osr as f32) * config.symbol_period as f32;
            print!("{}sec {}Hz {:?}\n",time_sec, freq_hz, c);
        }
    }
}