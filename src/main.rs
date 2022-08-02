use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;
use wav_io::header::*;
use wav_io::*;

mod constant;
mod crc;
mod ft8decode;
mod ft8encode;
mod gfsk;
mod ldpc;
mod monitor;
mod pack;
mod text;
mod unpack;

use crate::constant::{FT8_NN, FTX_LDPC_K_BYTES};
use crate::ft8decode::*;
use crate::ft8encode::*;
use crate::gfsk::{synth_gfsk, FT8_SYMBOL_BT};
use crate::monitor::Candidate;
use crate::monitor::{Config, Monitor};
use crate::pack::*;

fn main() {
    let args: Vec<String> = env::args().collect();

    let config = Config {
        sample_rate: 12000,
        symbol_period: 0.16f32,
        slot_time: 15.0f32,
        time_osr: 64,
        freq_osr: 2,
        sync_min_score: 10,
        num_threads: 16,
        ldpc_max_iteration: 20,
    };

    let mut samples = Vec::new();
    let mut header = WavHeader::new_mono();
    let mut packed = [0u8; FTX_LDPC_K_BYTES];
    let mut tones = [0usize; FT8_NN];

    if args.len() == 2 {
        // Input from file
        let input_wav = File::open(&args[1]).unwrap();
        (header, samples) = read_from_file(input_wav).unwrap();
        print!("{:?}", header);

        if header.channels >= 2 {
            samples = utils::stereo_to_mono(samples);
            header.channels = 1;
        }

        if header.sample_rate != config.sample_rate {
            samples = resample::linear(samples, 2, header.sample_rate, config.sample_rate);
            header.sample_rate = config.sample_rate;
        }

        let mut file_out = File::create("./resampled.wav").unwrap();
        writer::to_file(&mut file_out, &WavData::new(header, samples.clone())).unwrap();
    } else if args.len() == 3 {
        // Generate GSK symbol
        let frequency = args[1].parse::<f32>().unwrap();

        if pack77(&args[2], &mut packed) < 0 {
            println!("Cannot parse message! {}", &args[1]);
            return;
        }

        ft8_encode(&packed, &mut tones);

        print!("FSK tones: ");
        for t in tones.iter() {
            print!("{} ", t);
        }
        println!();

        let num_samples =
            (0.5 + FT8_NN as f32 * config.symbol_period * config.sample_rate as f32) as usize;
        let num_silence =
            ((config.slot_time * config.sample_rate as f32) as usize - num_samples) / 2;

        samples = vec![0.0; num_samples];

        synth_gfsk(
            &tones,
            FT8_NN,
            frequency,
            FT8_SYMBOL_BT,
            config.symbol_period,
            config.sample_rate as f32,
            &mut samples,
        );

        let mut silence_before = vec![0.0; num_silence];
        let mut silence_after = vec![0.0; num_silence];

        silence_before.append(&mut samples);
        silence_before.append(&mut silence_after);
        samples = silence_before;

        header.sample_rate = config.sample_rate;
        header.channels = 1;
        header.bits_per_sample = 16;
        header.sample_format = SampleFormat::Int;

        let mut file_out = File::create("./resampled.wav").unwrap();
        writer::to_file(&mut file_out, &WavData::new(header, samples.clone())).unwrap();
    } else {
        print!("Usage: rustft8 <wavfile> | <freq> <message> ");
        return;
    }

    print!(
        "Num. of Samples = {}.\nTime oversampling rate = {}.\nFrequency oversampling rate = {}.\n",
        samples.len(),
        config.time_osr,
        config.freq_osr
    );

    let mut mon = Monitor::new(&config, &samples);
    let start = Instant::now();

    mon.process_all();
    println!(
        "Num. of block = {}, Max mag = {} ({:?} elapsed.)",
        mon.wf.num_blocks,
        mon.max_mag,
        start.elapsed()
    );

    let time_osr_step = config.time_osr / config.num_threads;
    let wf = Arc::new(mon.wf);
    let config = Arc::new(config);
    let mut handles = vec![];
    let message_hash: Arc<Mutex<HashMap<u16, Message>>> = Arc::new(Mutex::new(HashMap::new()));
    println!("Spawning {} threads.", &config.num_threads);

    for time_sub_from in (0..config.time_osr).step_by(time_osr_step) {
        let wf = Arc::clone(&wf);
        let config = Arc::clone(&config);
        let message_hash = Arc::clone(&message_hash);
        let handle = thread::spawn(move || {
            let mut find_sync: FT8FindSync = FT8FindSync::new(&wf);
            let mut candidates: Vec<Candidate> = Vec::new();
            let _num = find_sync.ft8_find_sync(
                time_sub_from,
                time_sub_from + time_osr_step,
                config.sync_min_score,
                &mut candidates,
            );
            /*
            print!(
                "Costas sync founds {} candidates at {} ({:?} elapsed.)\n",
                _num,
                time_sub_from,
                start.elapsed()
            );
            */
            let decode = FT8Decode::new(&wf);
            let mut _success = 0;

            for c in candidates.iter() {
                let mut message = Message::new();
                if decode.ft8_decode(c, config.ldpc_max_iteration, &mut message) {
                    let freq_hz = (c.freq_offset as f32 + c.freq_sub as f32 / wf.freq_osr as f32)
                        / config.symbol_period as f32;
                    let time_sec = (c.time_offset as f32 + c.time_sub as f32 / wf.time_osr as f32)
                        * config.symbol_period as f32;

                    _success += 1;

                    message.df = freq_hz;
                    message.min_dt = time_sec;
                    message.max_dt = time_sec;

                    let mut message_hash = message_hash.lock().unwrap();
                    match message_hash.get_key_value(&message.hash) {
                        None => {
                            message_hash.insert(message.hash, message);
                        }
                        Some((_, v)) => {
                            if message.max_score < v.max_score {
                                message.max_score = v.max_score
                            }
                            if message.min_score > v.min_score {
                                message.min_score = v.min_score;
                            }
                            if v.min_dt > time_sec {
                                message.min_dt = time_sec;
                                message.max_dt = v.max_dt;
                                message_hash.insert(message.hash, message);
                            } else if time_sec > v.max_dt {
                                message.max_dt = time_sec;
                                message.min_dt = v.min_dt;
                                message_hash.insert(message.hash, message);
                            }
                        }
                    }
                }
            }
            /*
            print!(
                "{} candidates successfully decoded at {}. ({:?} elapsed.)\n",
                _success,
                time_sub_from,
                start.elapsed()
            );
            */
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap()
    }

    let messages = message_hash.lock().unwrap();
    println!(
        "Decoded messages: {} stations. ({:?} elapsed.)",
        messages.len(),
        start.elapsed()
    );
    for (i, v) in messages.values().enumerate() {
        //print!("{:?} diff DT={}ms\n", v, (v.max_dt - v.min_dt) * 1000.0);
        println!(
            "{}: {} {} {} {}",
            i + 1,
            v.max_score,
            v.min_dt,
            v.df,
            v.text
        );
    }
}
