use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use saykey::{audio::MAX_AUDIO_BYTES, config::Config, pipeline::Pipeline};
use std::{io::Read, path::PathBuf};

#[derive(Parser)]
#[command(version, about = "Windows hotkey dictation in Rust")]
struct Args {
    /// TOML configuration file; otherwise use built-in defaults.
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start Windows dictation: Ctrl+Alt+Space toggles, Ctrl+Alt+Esc cancels.
    Run,
    /// Transcribe an existing audio file and write text to stdout.
    Transcribe { file: PathBuf },
    /// Validate configuration without recording audio or contacting a provider.
    Check,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let config = Config::load(args.config.as_deref())?;
    match args.command {
        Command::Check => {
            config.validate()?;
            println!("Configuration valid. No audio recorded or API requests made.");
        }
        Command::Transcribe { file } => {
            let mut bytes = Vec::new();
            std::fs::File::open(&file)
                .context("Cannot open audio file")?
                .take(MAX_AUDIO_BYTES as u64 + 1)
                .read_to_end(&mut bytes)?;
            ensure!(bytes.len() <= MAX_AUDIO_BYTES, "Audio exceeds 24 MB");
            let name = file
                .file_name()
                .and_then(|name| name.to_str())
                .context("Invalid filename")?;
            let result = Pipeline::new(config)?.transcribe(bytes, name)?;
            if result.cleanup_failed {
                eprintln!("Cleanup failed; returning the original transcript.");
            }
            println!("{}", result.text);
        }
        Command::Run => {
            #[cfg(windows)]
            saykey::windows::run(config)?;
            #[cfg(not(windows))]
            anyhow::bail!("Live dictation requires Windows. Use transcribe or check on this OS.");
        }
    }
    Ok(())
}
