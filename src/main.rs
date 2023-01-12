use std::{error::Error, process::ExitCode};

use clap::{Parser, Subcommand};
use database::{ArchivedState, Database};
use inquire::{
    list_option::ListOption, validator::Validation, Confirm, CustomType, DateSelect, MultiSelect,
    Select,
};

mod database;
pub mod error;
mod model;
mod schema;

use crate::{
    database::TimeSpan,
    model::{Frame, Timestamp},
};

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

    /// Print the current project
    Current,

    /// Add a project
    NewProject { name: String },

    /// Add a tag
    NewTag { name: String },

    /// Tag projects interactively
    Tag,

    /// Analyze activities performed in a time frame
    Analyze(AnalyzeOptions),
}

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
            NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(23, 59, 59).unwrap(),
        )
    };

    let begin = Timestamp::from_naive(begin.and_time(start_time));
    let end = Timestamp::from_naive(end.and_time(end_time));
    Ok((begin, end))
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

fn stop_current_frame(db: &mut Database) -> Option<Frame> {
    if let Some(current) = db.stop().expect("Database is broken") {
        let duration = current.end.unwrap().0 - current.start.0;
        let project = db.lookup_project(current.project).expect("Database is broken").unwrap();

        println!(
            "Tracked time for Task {}: {}",
            project.name,
            duration.format()
        );

        Some(current)
    } else {
        None
    }
}

fn list_frames(db: &mut Database, span: TimeSpan) {
    let (start, end) = span;

    // TODO(texel, 2022-09-29): Remove this assert once the TimeSpan type guarantees that fact
    assert!(start < end);

    let data = db.get_frames_in_span(span, ArchivedState::Both).expect("Database is broken");

    for (project, frame) in data {
        if let Some(end) = frame.end {
            println!(
                "{}: {} -> {} ({})",
                project.name,
                frame.start.0,
                end.0,
                (end.0 - frame.start.0).format()
            );
        } else {
            println!(
                "{}: {} -> now ({})",
                project.name,
                frame.start.0,
                frame.start.elapsed().format()
            );
        }
    }
}

fn tag_inquire(database: &mut Database) {
    let mut possible_projects = database.all_projects(ArchivedState::NotArchived).expect("Database is broken");
    if possible_projects.is_empty() {
        println!("Please create a project before tagging.");
        return;
    }

    let mut possible_tags = database.all_tags(ArchivedState::NotArchived).expect("Database is broken");
    if possible_tags.is_empty() {
        println!("Please create a tag before tagging.");
        return;
    }

    let validator = |input: &[ListOption<&&String>]| {
        if input.is_empty() {
            Ok(Validation::Invalid("Select at least one element".into()))
        } else {
            Ok(Validation::Valid)
        }
    };

    let selected_projects: Vec<_> = MultiSelect::new(
        "Select the projects to tag",
        possible_projects.iter().map(|p| &p.name).collect(),
    )
    .with_validator(validator)
    .raw_prompt()
    .unwrap()
    .into_iter()
    .map(|item| item.index)
    .collect();

    let selected_tags: Vec<_> = MultiSelect::new(
        "Select the tags to apply to selected projects.",
        possible_tags.iter().map(|p| &p.name).collect(),
    )
    .with_validator(validator)
    .raw_prompt()
    .unwrap()
    .into_iter()
    .map(|item| item.index)
    .collect();

    database
        .tag_projects(
            pick(&mut possible_tags, &selected_tags),
            pick(&mut possible_projects, &selected_projects),
        )
        .expect("Could not tag projects.");
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let mut database = Database::new().unwrap();

    match cli.action {
        Action::Start => {
            let _ = stop_current_frame(&mut database);

            let mut possible_projects = database.all_projects(ArchivedState::NotArchived).expect("Database is broken");
            if possible_projects.is_empty() {
                println!("Please create a project before starting a task.");
                return ExitCode::FAILURE;
            }

            let selected_project = Select::new(
                "Select the project to start",
                possible_projects.iter().map(|p| &p.name).collect(),
            )
            .raw_prompt()
            .unwrap();

            let index = selected_project.index;
            let selected_project = &mut possible_projects[index];

            database
                .start(selected_project)
                .expect("Failed to start project");
        }
        Action::Stop => {
            let stopped_something = stop_current_frame(&mut database).is_some();

            if !stopped_something {
                println!("Nothing to do!");
            }
        }
        Action::NewProject { name } => {
            database
                .create_project(&name)
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

            list_frames(&mut database, span);
        }
        Action::NewTag { name } => {
            database.create_tag(&name).expect("Error creating tag");
        }
        Action::Tag => tag_inquire(&mut database),
        Action::Current => {
            let Ok(current) = database.current_frame() else {return ExitCode::FAILURE};
            let project = database
                .lookup_project(current.project).expect("Database is broken")
                .unwrap_or_else(|| panic!("Found no project for id {}", current.id()));

            let task = &project.name;
            println!("{}: {}", task, current.start.elapsed().format());
        }
    }

    ExitCode::SUCCESS
}

fn pick<T>(items: &mut Vec<T>, idxs: &[usize]) -> Vec<T> {
    // Move the items into a vector of Option<T> we can remove items from
    // without reordering.
    let mut opt_items: Vec<Option<T>> = items.drain(..).map(Some).collect();

    // Take the items.
    let picked: Vec<T> = idxs
        .iter()
        .map(|&i| opt_items[i].take().expect("duplicate index"))
        .collect();

    // Put the unpicked items back.
    items.extend(opt_items.into_iter().flatten());

    picked
}
