use monitor::Waterfall;
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
mod spectrogram;
mod text;
mod unpack;

use crate::constant::{FT8_NN, FT8_SLOT_TIME, FT8_SYMBOL_PERIOD, FTX_LDPC_K_BYTES};
use crate::ft8decode::*;
use crate::ft8encode::*;
use crate::gfsk::{synth_gfsk, FT8_SYMBOL_BT};
use crate::monitor::Candidate;
use crate::monitor::{Config, Monitor};
use crate::pack::*;

fn get_df(c: &Candidate, wf: &Waterfall) -> (f32, f32) {
    let freq_hz =
        (c.freq_offset as f32 + c.freq_sub as f32 / wf.freq_osr as f32) / FT8_SYMBOL_PERIOD as f32;
    let time_sec =
        (c.time_offset as f32 + c.time_sub as f32 / wf.time_osr as f32) * FT8_SYMBOL_PERIOD as f32;
    
        (freq_hz, time_sec)
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let config = Config {
        sample_rate: 12000,
        time_osr: 4,
        freq_osr: 2,
        sync_min_score: 10,
        num_threads: 8,
        ldpc_max_iteration: 20,
    };

    let mut samples= Vec::new();
    let mut header = WavHeader::new_mono();
    let mut packed = [0u8; FTX_LDPC_K_BYTES];
    let mut tones = [0usize; FT8_NN];

    if args.len() == 2 {
        // Input from file
        let input_wav = File::open(&args[1]).unwrap();
        (header, samples) = read_from_file(input_wav).unwrap();

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
    } else if args.len() > 2 {
        // Generate FT8 symbols and GFSK modulated samples.
        let frequency = args[1].parse::<f32>().unwrap();
		let attn = args[2].parse::<f32>().unwrap();
		let attn = 10.0_f32.powf(attn/20.0);

        if pack77(&args[3], &mut packed) < 0 {
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
            (0.5 + FT8_NN as f32 * FT8_SYMBOL_PERIOD * config.sample_rate as f32) as usize;
        let num_silence = ((FT8_SLOT_TIME * config.sample_rate as f32) as usize - num_samples) / 2;

        samples = vec![0.0; num_samples];

        synth_gfsk(
            &tones,
            FT8_NN,
            frequency,
            FT8_SYMBOL_BT,
            FT8_SYMBOL_PERIOD,
            config.sample_rate as f32,
            &mut samples,
        );

        let mut silence_before = vec![0.0; num_silence];
        let mut silence_after = vec![0.0; num_silence];

        silence_before.append(&mut samples);
        silence_before.append(&mut silence_after);
        samples = silence_before;

        samples = samples.iter().map(|x| * x * attn).collect::<Vec<_>>();

        header.sample_rate = config.sample_rate;
        header.channels = 1;
        header.bits_per_sample = 32;
        header.sample_format = SampleFormat::Float;

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
    mon.dump_spectrogram("./fft.png");
    
    println!(
        "Num. of block = {}, Max mag = {} ({:?} elapsed.)",
        mon.wf.num_blocks,
        mon.max_mag,
        start.elapsed()
    );

    let sched = mon.decode_frequencies(config.num_threads);
    let wf = Arc::new(mon.wf);
    let config = Arc::new(config);
    let mut handles = vec![];
    let message_hash: Arc<Mutex<HashMap<u16, Message>>> = Arc::new(Mutex::new(HashMap::new()));

    println!("Spawning {} threads for each frequencies ={:?}", &config.num_threads, sched);
    for (freq_from, freq_to) in sched {    
        let wf = Arc::clone(&wf);
        let config = Arc::clone(&config);
        let message_hash = Arc::clone(&message_hash);
        let handle = thread::spawn(move || {
            let mut find_sync: FT8FindSync = FT8FindSync::new(&wf);
            let mut candidates: Vec<Candidate> = Vec::new();
            let _num =
                find_sync.ft8_find_sync(freq_from, freq_to, config.sync_min_score, &mut candidates);
            let decode = FT8Decode::new(&wf);
            let mut success = 0;
            for c in candidates.iter() {
                let mut message = Message::new();
                if decode.ft8_decode(c, config.ldpc_max_iteration, &mut message) {
                    let (freq_hz, time_sec)  = get_df(c, &wf);
                    let mut message_hash = message_hash.lock().unwrap();
                    success += 1;
                    match message_hash.get_mut(&message.hash) {
                        None => {
                            message_hash.insert(message.hash, message);
                        }
                        Some(v) => {
                            v.df.push((c.score, time_sec, freq_hz));
                        }
                    }
                }
            }
            println!(
                "{:?} freq bin= {} - {} : deocdes {} messages from {} candidates.",
                thread::current().id(),
                freq_from,
                freq_to,
                success,
                candidates.len()
            );
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap()
    }

    let mut messages = message_hash.lock().unwrap();
    println!(
        "Decoded messages: {} stations. ({:?} elapsed.)",
        messages.len(),
        start.elapsed()
    );
    for (i, mesg) in messages.values_mut().enumerate() {
        let (score, df, dt) = mesg.df[0];
        println!(
            "{} : {}Hz {}s S={}: {}",
            i + 1,
            (dt * 10.0).round() / 10.0,
            (df * 10.0).round() / 10.0,
            score,
            mesg.text
        );
    }
}
