use std::{error::Error, env, io::{stdin, Read}};

use chrono::NaiveDateTime;
use clap::{Parser, Subcommand};
use diesel::{Queryable, SqliteConnection, Connection, prelude::*};
use dotenvy::dotenv;
use inquire::{DateSelect, Confirm, CustomType, MultiSelect, Select};
mod schema;
mod model;

use diesel_migrations::{embed_migrations, EmbeddedMigrations};
use model::NewProject;
use schema::projects;

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
    since_yesterday: bool
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
    Analyze (AnalyzeOptions)
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
            let options = (1i32..5i32).map(|i| i.to_string());

            let answers = Select::new("Select some numbers", options.collect()).prompt().unwrap();
            dbg!(answers);
        },
        Action::Stop => todo!(),
        Action::NewProject { name } => {
            let new_project = NewProject{name: &name};
             diesel::insert_into(projects::table)
                 .values(&new_project)
                 .execute(connection)
                 .expect("Error creating project")
                 ;
        },
        Action::Analyze ( options ) => {
            if options.is_interactive() {
                do_inquire_stuff().unwrap();
            } else {
                println!("No activities since yesterday, since we didn't implement tracking yet!");
            }
        },
    }
}
