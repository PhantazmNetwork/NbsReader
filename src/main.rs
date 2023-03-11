use std::{env, io, mem};
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{BufReader, ErrorKind, Read, Write};
use std::process::ExitCode;
use std::str::Utf8Error;

use serde::{Deserialize, Serialize};

#[macro_export]
macro_rules! read_num {
    ( $variable:ident,$number_type:ty ) => {
        {
            let mut bytes = [0; mem::size_of::<$number_type>()];
            $variable.read_exact(&mut bytes).map(|_| <$number_type>::from_le_bytes(bytes))
        }
    }
}

enum ReadError {
    Argument(String),
    Io(io::Error),
    Utf8(Utf8Error),
    Json(serde_json::Error)
}

impl ReadError {
    fn argument(string: impl Into<String>) -> Self {
        ReadError::Argument(string.into())
    }
}

impl From<String> for ReadError {
    fn from(message: String) -> Self {
        ReadError::Argument(message)
    }
}

impl From<io::Error> for ReadError {
    fn from(error: io::Error) -> Self {
        ReadError::Io(error)
    }
}

impl From<Utf8Error> for ReadError {
    fn from(error: Utf8Error) -> Self {
        ReadError::Utf8(error)
    }
}

impl From<serde_json::Error> for ReadError {
    fn from(error: serde_json::Error) -> Self {
        ReadError::Json(error)
    }
}

impl Display for ReadError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        return match self {
            ReadError::Argument(message) => write!(formatter, "{}", message),
            ReadError::Io(io_error) => write!(formatter, "IO-related error: {}", io_error),
            ReadError::Utf8(utf_8_error) => write!(formatter, "Invalid string: {}", utf_8_error),
            ReadError::Json(json_error) => write!(formatter, "Illegal generated JSON: {}", json_error)
        }
    }
}

fn read_utf8_string(read: &mut impl Read) -> Result<String, ReadError> {
    let length = read_num!(read, u32)?;
    if length == 0 {
        return Ok(String::from(""));
    }

    let mut string_buffer = Vec::with_capacity(length as usize);
    let mut itr = read.bytes();
    for _ in 0..length {
        match itr.next() {
            None => return Err(ReadError::Io(io::Error::new(ErrorKind::UnexpectedEof, "Encountered EOF while reading string"))),
            Some(next) => string_buffer.push(next?)
        }
    }

    return String::from_utf8(string_buffer).map_err(|err| ReadError::Utf8(err.utf8_error()));
}

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

fn main() -> ExitCode {
    return match parse() {
        Ok(_) => ExitCode::from(0),
        Err(error) => {
            println!("{}", error);
            ExitCode::from(1)
        }
    }
}

fn parse() -> Result<(), ReadError> {
    let program_args: Vec<String> = env::args().collect();
    if program_args.len() != 2 {
        return Err(ReadError::argument("Bad number of arguments"));
    }

    let mut target_dir = env::current_dir()?;

    if !target_dir.is_dir() || !target_dir.exists() {
        return Err(ReadError::argument("Current working directory does not exist"));
    }

    let relative_file = &program_args[1];
    target_dir.push(relative_file);

    if !target_dir.exists() {
        return Err(ReadError::argument(format!("Nonexistent file '{relative_file}'")));
    }

    let mut reader = BufReader::new(File::open(&target_dir)?);

    let proto = read_num!(reader, u16)?;
    if proto != 0 {
        return Err(ReadError::argument("Invalid file, or file was made using an unsupported version of NBS"));
    }

    let nbs_version = read_num!(reader, u8)?;
    let string = match nbs_version {
        5 => handle_version_5(reader)?,
        _ => return Err(ReadError::argument("File was made using an unsupported version of NBS"))
    };

    let mut path = target_dir.parent().ok_or_else(|| ReadError::argument("Missing parent directory"))?.to_path_buf();
    path.push("output.json");

    let mut output = File::create(path)?;
    return output.write_all(string.into_bytes().as_slice()).map_err(|err| ReadError::Io(err));
}

fn normalize_tick(tempo: u16, tick: u16) -> u16 {
    let tempo_ticks = tempo as f32 / 100.0;
    let res = tick as f32 * (20.0 / tempo_ticks);
    return res.round() as u16;
}

fn handle_version_5<T: Read>(mut buf: T) -> Result<String, ReadError> {
    let vanilla_instrument_count = read_num!(buf, u8)?;
    let song_length = read_num!(buf, u16)?;
    let layer_count = read_num!(buf, u16)?;

    let song_name = read_utf8_string(&mut buf)?;
    let song_author = read_utf8_string(&mut buf)?;
    let song_original_author = read_utf8_string(&mut buf)?;
    let song_description = read_utf8_string(&mut buf)?;
    let song_tempo = read_num!(buf, u16)?;

    let auto_saving = read_num!(buf, u8)?;
    let auto_saving_duration = read_num!(buf, u8)?;
    let time_signature = read_num!(buf, u8)?;

    let minutes_spent = read_num!(buf, u32)?;
    let left_clicks = read_num!(buf, u32)?;
    let right_clicks = read_num!(buf, u32)?;
    let note_blocks_added = read_num!(buf, u32)?;
    let note_blocks_removed = read_num!(buf, u32)?;

    let schematic_file_name = read_utf8_string(&mut buf)?;

    let loop_on = read_num!(buf, u8)?;
    let max_loop_count = read_num!(buf, u8)?;
    let loop_start_tick = read_num!(buf, u16)?;

    let mut notes = Vec::new();

    let mut actual_tick = u16::MAX;
    let mut last_actual_tick = 0;
    loop {
        let jumps_to_next_tick = read_num!(buf, u16)?;
        if jumps_to_next_tick == 0 {
            break;
        }

        actual_tick = actual_tick.wrapping_add(jumps_to_next_tick);

        let mut actual_layer = u16::MAX;
        let mut first = true;
        loop {
            let jumps_to_next_layer = read_num!(buf, u16)?;
            if jumps_to_next_layer == 0 {
                break
            }

            actual_layer = actual_layer.wrapping_add(jumps_to_next_layer);

            let delay_tick: u16 = match first {
                true => normalize_tick(song_tempo, actual_tick - last_actual_tick),
                false => 0
            };

            notes.push(Note {
                delay_ticks: delay_tick,
                layer: actual_layer,
                note_block_instrument: read_num!(buf, u8)?,
                note_block_key: read_num!(buf, u8)?,
                note_block_velocity: read_num!(buf, u8)?,
                note_block_panning: read_num!(buf, u8)?,
                note_block_pitch: read_num!(buf, i16)?
            });

            first = false;
        }

        last_actual_tick = actual_tick;
    }

    let mut layers = Vec::new();
    for _ in 0..layer_count {
        layers.push(Layer {
            layer_name: read_utf8_string(&mut buf)?,
            layer_lock: read_num!(buf, u8)?,
            layer_volume: read_num!(buf, u8)?,
            layer_stereo: read_num!(buf, u8)?
        });
    }

    let custom_instrument_count = read_num!(buf, u8)?;
    let mut custom_instruments = Vec::with_capacity(custom_instrument_count as usize);
    for _ in 0..custom_instrument_count {
        custom_instruments.push(CustomInstrument {
            instrument_name: read_utf8_string(&mut buf)?,
            sound_file: read_utf8_string(&mut buf)?,
            sound_pitch: read_num!(buf, u8)?,
            press_key: read_num!(buf, u8)?
        });
    }

    return Ok(serde_json::to_string(&Data {
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
    })?);
}