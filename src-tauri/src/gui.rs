use std::{process::ExitCode, sync::Mutex};

use crate::{
    database::Database,
    error::Result,
    model::{Frame, Project},
};

macro_rules! wrap {
    ($function_name:ident ($($par_name:ident :$par_type:ty),*) -> $return_type:ty) => {
        #[tauri::command]
        fn $function_name(database: tauri::State<'_, Mutex<Database>>, $($par_name: $par_type),*) -> $return_type {
            let mut db = database.lock().unwrap();
            db.$function_name($($par_name),*)
        }
    };
}

pub fn tauri_main(database: Database) -> ExitCode {
    tauri::Builder::default()
        .manage(Mutex::new(database))
        .invoke_handler(tauri::generate_handler![
            current_frame,
            lookup_project,
            start,
            stop
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
    ExitCode::SUCCESS
}

wrap!(current_frame () -> Result<Frame>);

wrap!(lookup_project (project_id: i32) -> Result<Option<Project>>);

wrap!(stop() -> Result<Option<Frame>>);

#[tauri::command]
fn start(
    database: tauri::State<'_, Mutex<Database>>,
    mut project: Project,
) -> Result<(Project, Frame)> {
    let mut db = database.lock().unwrap();
    let res = db.start(&mut project);
    Ok((project, res?))
}
