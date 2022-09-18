use std::{
    env,
    error::Error,
    io::{stdin, Read},
};

use chrono::NaiveDateTime;
use clap::{Parser, Subcommand};
use diesel::{prelude::*, Connection, Queryable, SqliteConnection};
use dotenvy::dotenv;
use inquire::{Confirm, CustomType, DateSelect, MultiSelect, Select};
mod model;
mod schema;

use diesel_migrations::{embed_migrations, EmbeddedMigrations};
use model::{NewProject, Project, Timestamp};
use schema::projects;

use crate::{model::NewFrame, schema::frames};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

pub fn establish_connection() -> SqliteConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
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

fn do_inquire_stuff() -> Result<(), Box<dyn Error>> {
    let begin = DateSelect::new("Enter start date");
    let begin = begin.prompt()?;
    let end = DateSelect::new("Enter end date").with_min_date(begin);
    let end = end.prompt()?;

    let precise_mode = Confirm::new("Do you want to enter start/end times?").prompt()?;

    if precise_mode {
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

        println!("{start_time} -> {end_time}");
    }

    println!("Time span: {}", end - begin);
    Ok(())
}

fn main() {
    let connection = &mut establish_connection();

    let cli = Cli::parse();
    match cli.action {
        Action::Start => {
            use crate::schema::projects::dsl::*;
            let mut possible_projects = projects
                .filter(archived.eq(false))
                .load::<Project>(connection)
                .expect("Failed to query database");

            possible_projects.sort_by(|a, b| b.last_access_time.cmp(&a.last_access_time));

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
        Action::Stop => todo!(),
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
            if options.is_interactive() {
                do_inquire_stuff().unwrap();
            } else {
                println!("No activities since yesterday, since we didn't implement tracking yet!");
            }
        }
    }
}
