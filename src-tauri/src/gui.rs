use std::{process::ExitCode, sync::Mutex};

use crate::{database::Database, error::Result, model::Frame};

pub fn tauri_main(database: Database) -> ExitCode {
    tauri::Builder::default()
        .manage(Mutex::new(database))
        .invoke_handler(tauri::generate_handler![current])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
    ExitCode::SUCCESS
}

#[tauri::command]
fn current(database: tauri::State<'_, Mutex<Database>>) -> Result<Frame> {
    let mut db = database.lock().unwrap();
    db.current_frame()
}
