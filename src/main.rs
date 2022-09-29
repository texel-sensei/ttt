use std::{env, error::Error, fs::create_dir_all};

use clap::{Parser, Subcommand};
use diesel::{prelude::*, Connection, SqliteConnection};
use dotenvy::dotenv;
use inquire::{Confirm, CustomType, DateSelect, Select};

use directories::ProjectDirs;

mod model;
mod schema;

use diesel_migrations::{embed_migrations, EmbeddedMigrations};
use model::{Frame, NewProject, Project, Timestamp};
use schema::projects;

use crate::{model::NewFrame, schema::frames};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

pub fn establish_connection() -> SqliteConnection {
    let database_url = if cfg!(debug_assertions) {
        dotenv().ok();

        env::var("DATABASE_URL").expect("DATABASE_URL must be set")
    } else {
        let dirs = ProjectDirs::from("", "", "ttt").expect("Failed to get base directory paths!");
        let data_folder = dirs.data_dir();

        create_dir_all(data_folder)
            .unwrap_or_else(|_| panic!("Failed to create data dir '{}'", data_folder.display()));

        data_folder
            .join("timetable.db")
            .to_str()
            .expect("Sorry non UTF-8 data directory names are not supported!")
            .to_owned()
    };

    let mut connection = SqliteConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url));

    use diesel_migrations::MigrationHarness;
    connection.run_pending_migrations(MIGRATIONS).unwrap();

    connection
}

#[derive(Parser)]
struct Cli {
    /// Action to perform
    #[clap(subcommand)]
    action: Action,
}

#[derive(Debug, Parser)]
struct AnalyzeOptions {
    /// Show the last 24h
    #[clap(short, long, action, default_value = "false")]
    since_yesterday: bool,
}

impl AnalyzeOptions {
    pub fn is_interactive(&self) -> bool {
        !self.since_yesterday
    }
}

#[derive(Subcommand, Debug)]
enum Action {
    /// Start tracking an activity
    Start,

    /// Stop tracking the current activity
    Stop,

    NewProject {
        name: String,
    },

    /// Analyze activities performed in a time frame
    Analyze(AnalyzeOptions),
}

type TimeSpan = (Timestamp, Timestamp);

fn do_inquire_stuff() -> Result<TimeSpan, Box<dyn Error>> {
    let begin = DateSelect::new("Enter start date");
    let begin = begin.prompt()?;
    let end = DateSelect::new("Enter end date").with_min_date(begin);
    let end = end.prompt()?;

    let precise_mode = Confirm::new("Do you want to enter start/end times?").prompt()?;

    let (start_time, end_time) = if precise_mode {
        let start_time: chrono::naive::NaiveTime = CustomType::new("Enter start time").prompt()?;
        let end_time: chrono::naive::NaiveTime = CustomType::new("Enter end time")
            .with_parser(&|text| {
                let time = text.parse().map_err(|_| ())?;
                if end == begin && time < start_time {
                    return Err(());
                }
                Ok(time)
            })
            .with_error_message(&format!("Enter a valid time that's after {start_time}!"))
            .prompt()?;
        (start_time, end_time)
    } else {
        use chrono::NaiveTime;
        (
            NaiveTime::from_hms(0, 0, 0),
            NaiveTime::from_hms(23, 59, 59),
        )
    };

    let begin = Timestamp::from_naive(begin.and_time(start_time));
    let end = Timestamp::from_naive(end.and_time(end_time));
    Ok((begin, end))
}

fn get_current_frame(connection: &mut SqliteConnection) -> Option<Frame> {
    use crate::schema::frames::dsl::*;
    let current = frames.filter(end.is_null()).load::<Frame>(connection);
    current.ok().and_then(|mut f| f.pop())
}

trait DurationExt {
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

fn stop_frame(connection: &mut SqliteConnection, frame: &mut Frame) {
    use crate::schema::projects::dsl::*;
    let now = Timestamp::now();
    frame.end = Some(now);
    diesel::update(&*frame)
        .set(&*frame)
        .execute(connection)
        .expect("Failed to update frame");
    let mut project = projects
        .filter(id.eq(frame.project))
        .load::<Project>(connection)
        .expect("Failed to query database")
        .pop()
        .unwrap_or_else(|| panic!("Found no project for id {}", frame.id));
    let task = &project.name;
    let duration = frame.end.unwrap().0 - frame.start.0;
    project.last_access_time = now;
    diesel::update(&project)
        .set(&project)
        .execute(connection)
        .expect("Failed to update project access time");

    println!("Tracked time for Task {}: {}", task, duration.format());
}

fn list_frames(connection: &mut SqliteConnection, span: TimeSpan) {
    let (start, end) = span;

    // TODO(texel, 2022-09-29): Remove this assert once the TimeSpan type guarantees that fact
    assert!(start < end);

    let data = frames::table
        .inner_join(projects::table)
        .select((frames::start, frames::end, projects::name))
        .filter(frames::end.ge(start))
        .or_filter(frames::end.is_null())
        .filter(frames::start.lt(end))
        .load::<(Timestamp, Option<Timestamp>, String)>(connection)
        .expect("Will definitely go wrong");

    for (start, end, name) in data {
        if let Some(end) = end {
            println!(
                "{}: {} -> {} ({})",
                name,
                start.0,
                end.0,
                (end.0 - start.0).format()
            );
        } else {
            println!(
                "{}: {} -> now ({})",
                name,
                start.0,
                start.elapsed().format()
            );
        }
    }
}

fn main() {
    let connection = &mut establish_connection();

    let cli = Cli::parse();
    match cli.action {
        Action::Start => {
            if let Some(mut current) = get_current_frame(connection) {
                stop_frame(connection, &mut current)
            }
            use crate::schema::projects::dsl::*;
            let mut possible_projects = projects
                .filter(archived.eq(false))
                .load::<Project>(connection)
                .expect("Failed to query database");

            possible_projects.sort_by(|a, b| b.last_access_time.cmp(&a.last_access_time));
            if possible_projects.is_empty() {
                println!("Please create a project before starting a task.");
                return;
            }

            let selected_project = Select::new(
                "Select the project to start",
                possible_projects.iter().map(|p| &p.name).collect(),
            )
            .raw_prompt()
            .unwrap();

            let index = selected_project.index;
            let selected_project = &mut possible_projects[index];

            let now = Timestamp::now();
            let frame = NewFrame {
                project: selected_project.id,
                start: &now,
                end: None,
            };
            diesel::insert_into(frames::table)
                .values(&frame)
                .execute(connection)
                .expect("Failed to insert frame into database");

            selected_project.last_access_time = now;
            diesel::update(&*selected_project)
                .set(&*selected_project)
                .execute(connection)
                .expect("Failed to update project access time");
        }
        Action::Stop => {
            if let Some(mut current) = get_current_frame(connection) {
                stop_frame(connection, &mut current)
            } else {
                println!("Nothing to do!");
            }
        }
        Action::NewProject { name } => {
            let new_project = NewProject {
                name: &name,
                last_access_time: &Timestamp::now(),
            };
            diesel::insert_into(projects::table)
                .values(&new_project)
                .execute(connection)
                .expect("Error creating project");
        }
        Action::Analyze(options) => {
            let span = if options.is_interactive() {
                do_inquire_stuff().unwrap()
            } else {
                // todo: handle commandline options in detail, assuming "since_yesterday" for now
                let end = Timestamp::now();
                let start = Timestamp(end.0 - chrono::Duration::days(1));
                (start, end)
            };

            list_frames(connection, span);
        }
    }
}
