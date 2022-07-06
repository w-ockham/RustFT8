mod constant;
mod monitor;
mod ft8decode;
mod ldpc;
mod crc;
mod unpack;
mod text;

use std::env;
use std::fs::File;
use std::time::Instant;
use std::collections::HashMap;
use monitor::{Monitor, Config};
use ft8decode::{*};

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
        sample_rate : header.sample_rate,
        symbol_period: 0.16f32,
        slot_time : 15.0f32,
        time_osr : 160,
        freq_osr : 2,
    };
    print!("Time oversampling rate = {}.\nFrequency oversampling rate = {}.\n",config.time_osr, config.freq_osr);
    
    let mut mon = Monitor::new(&config, &samples);
    let start = Instant::now();
    mon.process_all();
    print!("Max mag = {} ({:?} elapsed.)\n",mon.max_mag, start.elapsed());

    let mut find_sync:FT8FindSync = FT8FindSync::new(&mon.wf);
    let num = find_sync.ft8_find_sync(10);
    print!("Costas sync founds {} candidates. ({:?} elapsed.)\n",num, start.elapsed());

    let decode = FT8Decode::new(&mon.wf);
    let mut message_hash:HashMap<u16, Message> = HashMap::new();
    let mut success = 0;

    for c in find_sync.candidates.iter() {
        let mut message = Message::new();
        if decode.ft8_decode(c, 20, &mut message) {
            let freq_hz = (c.freq_offset as f32 + c.freq_sub as f32 / mon.wf.freq_osr as f32) / config.symbol_period as f32;
            let time_sec = (c.time_offset as f32 + c.time_sub as f32 / mon.wf.time_osr as f32) * config.symbol_period as f32;

            //print!("LDPC/CRC OK:{}sec {}Hz {:?}\n",time_sec, freq_hz, c);
            success+=1;
            message.df = freq_hz;
            message.min_dt = time_sec;
            message.max_dt = time_sec;

            match message_hash.get_key_value(&message.hash) {
                None => {message_hash.insert(message.hash, message);},
                Some((_,v)) => {
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
    print!("{} candidates successfully decoded. ({:?} elapsed.)\n", success, start.elapsed());
    
    print!("Decoded messages:\n");
    for v in message_hash.values() {
        print!("{:?} diff DT={}ms\n", v, (v.max_dt-v.min_dt)*1000.0);
    }
}