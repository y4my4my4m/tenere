//! Module for controlling the Raifus application via a control file

use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use log::debug;

/// Available commands for the Raifus application
#[derive(Debug, Clone, Copy)]
pub enum RaifusCommand {
    Next,
    Color,
    Play,
    Stop,
    SwitchIdle,
    SwitchTalking,
    SwitchNormal,
    Quit,
}

impl RaifusCommand {
    /// Convert the command to its string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            RaifusCommand::Next => "next",
            RaifusCommand::Color => "color",
            RaifusCommand::Play => "play",
            RaifusCommand::Stop => "stop",
            RaifusCommand::SwitchIdle => "switch_idle",
            RaifusCommand::SwitchTalking => "switch_talking",
            RaifusCommand::SwitchNormal => "switch_normal",
            RaifusCommand::Quit => "quit",
        }
    }
}

/// Default path for the Raifus control file
pub const DEFAULT_CONTROL_PATH: &str = "/tmp/raifus_control";

/// Send a command to the Raifus application
pub fn send_command(command: RaifusCommand) -> std::io::Result<()> {
    send_command_to_path(command, Path::new(DEFAULT_CONTROL_PATH))
}

/// Send a command to the Raifus application using a specific control file path
pub fn send_command_to_path(command: RaifusCommand, path: &Path) -> std::io::Result<()> {
    match File::create(path) {
        Ok(mut file) => {
            file.write_all(command.as_str().as_bytes())?;
            debug!("Sent Raifus command: {:?}", command);
            Ok(())
        },
        Err(e) => {
            // Don't fail if the control file can't be created - Raifus might not be running
            debug!("Failed to send Raifus command: {}", e);
            Ok(())
        }
    }
}

/// Signal that talking has started
pub fn signal_talking() -> std::io::Result<()> {
    send_command(RaifusCommand::SwitchTalking)
}

/// Signal that talking has ended (idle state)
pub fn signal_idle() -> std::io::Result<()> {
    send_command(RaifusCommand::SwitchIdle)
}
