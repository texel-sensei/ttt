use std::{process::ExitCode, sync::Mutex};

use crate::{
    database::Database,
    error::Result,
    model::{Frame, Project},
};

pub fn tauri_main(database: Database) -> ExitCode {
    tauri::Builder::default()
        .manage(Mutex::new(database))
        .invoke_handler(tauri::generate_handler![current, lookup_project])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
    ExitCode::SUCCESS
}

#[tauri::command]
fn current(database: tauri::State<'_, Mutex<Database>>) -> Result<Frame> {
    let mut db = database.lock().unwrap();
    db.current_frame()
}

#[tauri::command]
fn lookup_project(
    database: tauri::State<'_, Mutex<Database>>,
    project_id: i32,
) -> Result<Option<Project>> {
    let mut db = database.lock().unwrap();
    db.lookup_project(project_id)
}
