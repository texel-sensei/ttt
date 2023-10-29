// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::process::ExitCode;

use clap::Parser;

use crate::cli::{cli_main, Cli};
use crate::database::Database;
use crate::gui::tauri_main;

mod cli;
mod database;
pub mod error;
mod gui;
mod model;
mod schema;
mod timespan_parser;

pub trait DurationExt {
    fn format(&self) -> String;
}

impl DurationExt for chrono::Duration {
    fn format(&self) -> String {
        use std::fmt::Write as _;
        let mut mydur = *self;
        let mut result = String::new();

        let n = mydur.num_weeks();
        if n > 0 {
            let _ = write!(result, "{}w", n);
            mydur = mydur - Self::weeks(n);
        }
        let n = mydur.num_days();
        if n > 0 {
            if !result.is_empty() {
                result.push(' ');
            }
            let _ = write!(result, "{}d", n);
            mydur = mydur - Self::days(n);
        }
        let n = mydur.num_hours();
        if n > 0 {
            if !result.is_empty() {
                result.push(' ');
            }
            let _ = write!(result, "{}h", n);
            mydur = mydur - Self::hours(n);
        }
        let n = mydur.num_minutes();
        if n > 0 {
            if !result.is_empty() {
                result.push(' ');
            }
            let _ = write!(result, "{}min", n);
            mydur = mydur - Self::minutes(n);
        }
        let n = mydur.num_seconds();
        if n > 0 {
            if !result.is_empty() {
                result.push(' ');
            }
            let _ = write!(result, "{}s", n);
        }
        result
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let database = Database::new().unwrap();

    if cli.action.is_some() {
        cli_main(database, cli)
    } else {
        tauri_main(database)
    }
}
