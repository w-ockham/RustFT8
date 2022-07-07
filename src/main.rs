use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

mod constant;
mod crc;
mod ft8decode;
mod ldpc;
mod monitor;
mod text;
mod unpack;

use crate::monitor::Candidate;
use crate::ft8decode::*;
use crate::monitor::{Config, Monitor};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        print!("Usage: rustft8 <wavfile>\n");
        return;
    }

    let input_wav = File::open(&args[1]).unwrap();
    let (header, samples) = wav_io::read_from_file(input_wav).unwrap();
    println!("{:?} {}", header, header.sample_rate);

    let config = Config {
        sample_rate: header.sample_rate,
        symbol_period: 0.16f32,
        slot_time: 15.0f32,
        time_osr: 64,
        freq_osr: 2,
        sync_min_score: 10,
        num_threads: 16,
        ldpc_max_iteration: 20,
    };

    print!(
        "Time oversampling rate = {}.\nFrequency oversampling rate = {}.\n",
        config.time_osr, config.freq_osr
    );

    let mut mon = Monitor::new(&config, &samples);
    let start = Instant::now();

    mon.process_all();
    print!(
        "Max mag = {} ({:?} elapsed.)\n",
        mon.max_mag,
        start.elapsed()
    );

    let time_osr_step = config.time_osr / config.num_threads;
    let wf = Arc::new(mon.wf);
    let config = Arc::new(config);
    let mut handles = vec![];
    let message_hash: Arc<Mutex<HashMap<u16, Message>>> = Arc::new(Mutex::new(HashMap::new()));
    print!("Spawning {} threads.\n", &config.num_threads);

    for time_sub_from in (0..config.time_osr).step_by(time_osr_step) {
        let wf = Arc::clone(&wf);
        let config = Arc::clone(&config);
        let message_hash = Arc::clone(&message_hash);
        let handle = thread::spawn(move || {
            let mut find_sync: FT8FindSync = FT8FindSync::new(&wf);
            let mut candidates: Vec<Candidate> = Vec::new();
            let num = find_sync.ft8_find_sync(
                time_sub_from,
                time_sub_from + time_osr_step,
                config.sync_min_score,
                &mut candidates,
            );
            print!(
                "Costas sync founds {} candidates at {} ({:?} elapsed.)\n",
                num,
                time_sub_from,
                start.elapsed()
            );

            let decode = FT8Decode::new(&wf);
            let mut success = 0;

            for c in candidates.iter() {
                let mut message = Message::new();
                if decode.ft8_decode(c, config.ldpc_max_iteration, &mut message) {
                    let freq_hz = (c.freq_offset as f32 + c.freq_sub as f32 / wf.freq_osr as f32)
                        / config.symbol_period as f32;
                    let time_sec = (c.time_offset as f32 + c.time_sub as f32 / wf.time_osr as f32)
                        * config.symbol_period as f32;

                    success += 1;

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
            print!(
                "{} candidates successfully decoded at {}. ({:?} elapsed.)\n",
                success,
                time_sub_from,
                start.elapsed()
            );
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap()
    }

    let messages = message_hash.lock().unwrap();
    print!(
        "Decoded messages: {} stations. ({:?} elapsed.)\n",
        messages.len(),
        start.elapsed()
    );
    for v in messages.values() {
        print!("{:?} diff DT={}ms\n", v, (v.max_dt - v.min_dt) * 1000.0);
    }
}
