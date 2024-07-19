use std::env;
use std::fs;
use std::io::{self, BufReader, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use walkdir::WalkDir;
use rodio::{Decoder, OutputStream, Sink};
use crossterm::{
    event::{self, KeyCode},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    cursor,
};
use ctrlc;
use rodio::Source;

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
    let file = fs::File::open(file_path)?;
    let source = Decoder::new(BufReader::new(file))?;
    let duration = source.total_duration().unwrap_or(Duration::new(0, 0));
    let start_time = Instant::now();
    sink.lock().unwrap().append(source);

    // Handle pausing, resuming and progress bar
    loop {
        if is_paused.load(Ordering::SeqCst) {
            sink.lock().unwrap().pause();
        } else {
            sink.lock().unwrap().play();
        }

        // Display progress bar
        let elapsed = start_time.elapsed().as_secs();
        let total = duration.as_secs();
        if total > 0 {
            let progress = elapsed as f64 / total as f64;
            print_progress_bar(progress, elapsed, total);
        }

        thread::sleep(Duration::from_millis(100));

        if elapsed >= total {
            break;
        }
    }

    Ok(())
}

fn print_progress_bar(progress: f64, elapsed: u64, total: u64) {
    let bar_length = 50;
    let filled_length = (bar_length as f64 * progress) as usize;
    let bar: String = "=".repeat(filled_length) + &"-".repeat(bar_length - filled_length);

    print!(
        "\r[{}] {:02}:{:02}/{:02}:{:02}",
        bar,
        elapsed / 60,
        elapsed % 60,
        total / 60,
        total % 60
    );
    io::stdout().flush().unwrap();
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
    let is_paused_clone = Arc::clone(&is_paused);

    thread::spawn(move || {
        play_music(selected_file, is_paused_clone, sink_clone).expect("Error playing music");
    });

    // Terminal setup for UI
    execute!(io::stdout(), EnterAlternateScreen)?;
    terminal::enable_raw_mode()?;
    execute!(io::stdout(), cursor::Hide)?;

    // Handle key events for pausing, resuming, and exiting
    loop {
        if event::poll(Duration::from_millis(100))? {
            if let event::Event::Key(key_event) = event::read()? {
                match key_event.code {
                    KeyCode::Char('p') => {
                        let paused = is_paused.load(Ordering::SeqCst);
                        is_paused.store(!paused, Ordering::SeqCst);
                    }
                    KeyCode::Esc => break,
                    _ => {}
                }
            }
        }
    }

    // Cleanup
    terminal::disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    execute!(io::stdout(), cursor::Show)?;
    println!("\nExiting...");
    Ok(())
}
