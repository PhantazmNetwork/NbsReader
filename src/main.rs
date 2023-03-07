use std::env;
use std::fs::File;
use std::io::{BufReader, Read, Write};

use serde::{Deserialize, Serialize};

pub trait PrimitiveRead: Read {
    fn read_u8(&mut self) -> u8 {
        let mut bytes: [u8; 1] = [0; 1];
        self.read_exact(&mut bytes).expect("Error reading u8");
        return u8::from_le_bytes(bytes);
    }

    fn read_u16(&mut self) -> u16 {
        let mut bytes: [u8; 2] = [0; 2];
        self.read_exact(&mut bytes).expect("Error reading u16");
        return u16::from_le_bytes(bytes);
    }

    fn read_i16(&mut self) -> i16 {
        let mut bytes: [u8; 2] = [0; 2];
        self.read_exact(&mut bytes).expect("Error reading i16");
        return i16::from_le_bytes(bytes);
    }

    fn read_u32(&mut self) -> u32 {
        let mut bytes: [u8; 4] = [0; 4];
        self.read_exact(&mut bytes).expect("Error reading u32");
        return u32::from_le_bytes(bytes);
    }

    fn read_utf8_string(&mut self) -> String {
        let length = self.read_u32();
        if length == 0 {
            return "".to_string();
        }

        let mut string_buffer = Vec::with_capacity(length as usize);
        let mut itr = self.bytes();
        for _ in 0..length {
            string_buffer.push(itr.next().expect("Failed to read byte").unwrap());
        }

        return String::from_utf8(string_buffer).expect("Failed to read string");
    }
}

impl PrimitiveRead for BufReader<File> {}

#[derive(Serialize, Deserialize)]
struct Note {
    delay_ticks: u16,
    layer: u16,
    note_block_instrument: u8,
    note_block_key: u8,
    note_block_velocity: u8,
    note_block_panning: u8,
    note_block_pitch: i16
}

#[derive(Serialize, Deserialize)]
struct Layer {
    layer_name: String,
    layer_lock: u8,
    layer_volume: u8,
    layer_stereo: u8
}

#[derive(Serialize, Deserialize)]
struct CustomInstrument {
    instrument_name: String,
    sound_file: String,
    sound_pitch: u8,
    press_key: u8
}

#[derive(Serialize, Deserialize)]
struct Data {
    version: u8,

    vanilla_instrument_count: u8,
    song_length: u16,
    layer_count: u16,

    song_name: String,
    song_author: String,
    song_original_author: String,
    song_description: String,
    song_tempo: u16,

    auto_saving: u8,
    auto_saving_duration: u8,
    time_signature: u8,

    minutes_spent: u32,
    left_clicks: u32,
    right_clicks: u32,
    note_blocks_added: u32,
    note_blocks_removed: u32,

    schematic_file_name: String,

    loop_on: u8,
    max_loop_count: u8,
    loop_start_tick: u16,

    notes: Vec<Note>,
    layers: Vec<Layer>,
    custom_instruments: Vec<CustomInstrument>
}

fn main() {
    let program_args: Vec<String> = env::args().collect();
    if program_args.len() != 2 {
        panic!("Bad number of arguments");
    }

    let mut target_dir = env::current_dir()
        .expect("Error getting current directory");

    if !target_dir.is_dir() || !target_dir.exists() {
        panic!("Current working directory does not exist");
    }

    let relative_file = &program_args[1];
    target_dir.push(relative_file);

    if !target_dir.exists() {
        panic!("Nonexistent file '{relative_file}'");
    }

    let mut reader = BufReader::new(File::open(&target_dir)
        .expect("Failed to open target file"));

    let proto = reader.read_u16();
    if proto != 0 {
        panic!("Invalid file, or file was made using an unsupported version of NBS");
    }

    let nbs_version = reader.read_u8();
    let string = match nbs_version {
        5 => handle_version_5(reader),
        _ => panic!("File was made using an unsupported version of NBS")
    };

    let mut path = target_dir.parent().expect("Failed to find parent directory")
        .to_path_buf();
    path.push("output.json");

    let mut output = File::create(path).expect("Failed to create output file");
    output.write_all(string.into_bytes().as_slice()).expect("Failed to write output file");
}

fn normalize_tick(tempo: u16, tick: u16) -> u16 {
    let tempo_ticks = tempo as f32 / 100.0;
    let res = tick as f32 * (tempo_ticks / 20.0);
    return res.round() as u16;
}

fn handle_version_5(mut buf: BufReader<File>) -> String {
    let vanilla_instrument_count = buf.read_u8();
    let song_length = buf.read_u16();
    let layer_count = buf.read_u16();

    let song_name = buf.read_utf8_string();
    let song_author = buf.read_utf8_string();
    let song_original_author = buf.read_utf8_string();
    let song_description = buf.read_utf8_string();
    let song_tempo = buf.read_u16();

    let auto_saving = buf.read_u8();
    let auto_saving_duration = buf.read_u8();
    let time_signature = buf.read_u8();

    let minutes_spent = buf.read_u32();
    let left_clicks = buf.read_u32();
    let right_clicks = buf.read_u32();
    let note_blocks_added = buf.read_u32();
    let note_blocks_removed = buf.read_u32();

    let schematic_file_name = buf.read_utf8_string();

    let loop_on = buf.read_u8();
    let max_loop_count = buf.read_u8();
    let loop_start_tick = buf.read_u16();

    let mut notes = Vec::new();

    let mut actual_tick = u16::MAX;
    let mut last_actual_tick = 0;
    loop {
        let jumps_to_next_tick = buf.read_u16();
        if jumps_to_next_tick == 0 {
            break;
        }

        actual_tick = actual_tick.wrapping_add(jumps_to_next_tick);

        let mut actual_layer = u16::MAX;
        let mut first = true;
        loop {
            let jumps_to_next_layer = buf.read_u16();
            if jumps_to_next_layer == 0 {
                break
            }

            actual_layer = actual_layer.wrapping_add(jumps_to_next_layer);

            let delay_tick: u16 = if first { normalize_tick(song_tempo, actual_tick - last_actual_tick) } else { 0 };
            notes.push(Note {
                delay_ticks: delay_tick,
                layer: actual_layer,
                note_block_instrument: buf.read_u8(),
                note_block_key: buf.read_u8(),
                note_block_velocity: buf.read_u8(),
                note_block_panning: buf.read_u8(),
                note_block_pitch: buf.read_i16()
            });

            first = false;
        }

        last_actual_tick = actual_tick;
    }

    let mut layers = Vec::new();
    for _ in 0..layer_count {
        layers.push(Layer {
            layer_name: buf.read_utf8_string(),
            layer_lock: buf.read_u8(),
            layer_volume: buf.read_u8(),
            layer_stereo: buf.read_u8()
        });
    }

    let custom_instrument_count = buf.read_u8();
    let mut custom_instruments = Vec::with_capacity(custom_instrument_count as usize);
    for _ in 0..custom_instrument_count {
        custom_instruments.push(CustomInstrument {
            instrument_name: buf.read_utf8_string(),
            sound_file: buf.read_utf8_string(),
            sound_pitch: buf.read_u8(),
            press_key: buf.read_u8(),
        });
    }

    return serde_json::to_string(&Data {
        version: 5,
        vanilla_instrument_count,
        song_length,
        layer_count,
        song_name,
        song_author,
        song_original_author,
        song_description,
        song_tempo,
        auto_saving,
        auto_saving_duration,
        time_signature,
        minutes_spent,
        left_clicks,
        right_clicks,
        note_blocks_added,
        note_blocks_removed,
        schematic_file_name,
        loop_on,
        max_loop_count,
        loop_start_tick,
        notes,
        layers,
        custom_instruments
    }).unwrap();
}