use std::env;
use std::fs;
use std::io::{self, BufReader};
use std::path::Path;
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;
use rodio::{Decoder, OutputStream, Sink};
use std::thread;
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use ctrlc;

fn list_music_files(path: &Path) -> Vec<String> {
    let mut music_files = Vec::new();
    for entry in WalkDir::new(path) {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("flac") {
            music_files.push(path.to_string_lossy().into_owned());
        }
    }
    music_files
}

fn play_music(file_path: String, is_paused: Arc<AtomicBool>, sink: Arc<Mutex<Sink>>) -> Result<(), Box<dyn std::error::Error>> {
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let sink_clone = Arc::clone(&sink);

    let file = fs::File::open(file_path)?;
    let source = Decoder::new(BufReader::new(file))?;
    sink_clone.lock().unwrap().append(source);

    // Handle pausing and resuming
    loop {
        if is_paused.load(Ordering::SeqCst) {
            sink_clone.lock().unwrap().pause();
        } else {
            sink_clone.lock().unwrap().play();
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <SD card path>", args[0]);
        return Ok(());
    }

    let sd_card_path = Path::new(&args[1]);
    if !sd_card_path.exists() {
        eprintln!("The provided path does not exist.");
        return Ok(());
    }

    let music_files = list_music_files(sd_card_path);
    if music_files.is_empty() {
        println!("No music files found in the provided path.");
        return Ok(());
    }

    println!("Found the following music files:");
    for (index, file) in music_files.iter().enumerate() {
        println!("{}: {}", index, file);
    }

    println!("Enter the number of the file you want to play:");
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let selection: usize = input.trim().parse().expect("Please enter a valid number");

    if selection >= music_files.len() {
        eprintln!("Invalid selection.");
        return Ok(());
    }

    let selected_file = music_files[selection].clone();
    println!("Playing {}", selected_file);

    let is_paused = Arc::new(AtomicBool::new(false));
    let (_stream, stream_handle) = OutputStream::try_default().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let sink = Arc::new(Mutex::new(Sink::try_new(&stream_handle).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?));

    // Set up Ctrl+C handler
    {
        let is_paused = Arc::clone(&is_paused);
        ctrlc::set_handler(move || {
            let paused = is_paused.load(Ordering::SeqCst);
            is_paused.store(!paused, Ordering::SeqCst);
        }).expect("Error setting Ctrl-C handler");
    }

    let sink_clone = Arc::clone(&sink);
    thread::spawn(move || {
        play_music(selected_file, Arc::clone(&is_paused), sink_clone).expect("Error playing music");
    });

    // Keep the main thread alive while the music plays
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
