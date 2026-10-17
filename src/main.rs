mod ai;
mod app;
mod audio;
mod cli;
mod config;
mod error;
mod ipc;
mod practice;
mod privacy;
mod storage;
mod transcription;
mod ui;

use std::process::ExitCode;

fn main() -> ExitCode {
    let action = match cli::parse(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!();
            eprintln!("{}", cli::help_text());
            return ExitCode::from(2);
        }
    };

    match action {
        cli::CliAction::Help => {
            println!("{}", cli::help_text());
            ExitCode::SUCCESS
        }
        cli::CliAction::Version => {
            println!("niri-ai-copilot {}", cli::VERSION);
            ExitCode::SUCCESS
        }
        cli::CliAction::Ipc(cmd) => match ipc::send_command(cmd.as_str()) {
            Ok(reply) => {
                println!("{reply}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::from(1)
            }
        },
        cli::CliAction::Run { smoke } => match app::run(smoke) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::from(1)
            }
        },
    }
}
