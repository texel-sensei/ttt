use std::{error::Error, process::ExitCode};

use clap::{Parser, Subcommand};
use diesel::{prelude::*, SqliteConnection};
use inquire::{
    list_option::ListOption, validator::Validation, Confirm, CustomType, DateSelect, MultiSelect,
    Select,
};
use itertools::iproduct;


mod model;
mod schema;
mod database;
pub mod error;


use model::{Frame, NewProject, NewTag, Project, Timestamp};
use schema::{projects, tags};

use crate::{
    model::{HasAccessTime, NewFrame, Tag, TagProject},
    schema::{frames, tags_per_project},
    database::establish_connection,
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
            NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(23, 59, 59).unwrap(),
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
    let duration = frame.end.unwrap().0 - frame.start.0;

    project
        .touch(connection, &now)
        .expect("Failed to update project access time");

    let task = &project.name;
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

fn tag_inquire(connection: &mut SqliteConnection) {
    use crate::schema::projects::dsl::*;
    let mut possible_projects = projects
        .filter(schema::projects::dsl::archived.eq(false))
        .load::<Project>(connection)
        .expect("Failed to query database");

    possible_projects.sort_by(|a, b| b.last_access_time.cmp(&a.last_access_time));
    if possible_projects.is_empty() {
        println!("Please create a project before tagging.");
        return;
    }

    use crate::schema::tags::dsl::*;
    let mut possible_tags = tags
        .filter(schema::tags::dsl::archived.eq(false))
        .load::<Tag>(connection)
        .expect("Failed to query database");

    possible_tags.sort_by(|a, b| b.last_access_time.cmp(&a.last_access_time));
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

    // TODO(texel, 2022-10-26): Optimize to use a single update statement
    for selected in &selected_projects {
        possible_projects[*selected]
            .touch_now(connection)
            .expect("Failed to update access time");
    }
    for selected in &selected_tags {
        possible_tags[*selected]
            .touch_now(connection)
            .expect("Failed to update access time");
    }

    let selected_projects = selected_projects.into_iter().map(|i| &possible_projects[i]);
    let selected_tags = selected_tags.into_iter().map(|i| &possible_tags[i]);

    let combination: Vec<_> = iproduct!(selected_projects, selected_tags)
        .map(|(p, t)| TagProject {
            project_id: p.id,
            tag_id: t.id,
        })
        .collect();

    diesel::insert_or_ignore_into(tags_per_project::table)
        .values(combination)
        .execute(connection)
        .expect("Failed to store tags in database");
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let connection = &mut establish_connection().unwrap();

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

            selected_project
                .touch(connection, &now)
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
        Action::NewTag { name } => {
            let new_tag = NewTag {
                name: &name,
                last_access_time: &Timestamp::now(),
            };
            diesel::insert_into(tags::table)
                .values(&new_tag)
                .execute(connection)
                .expect("Error creating project");
        }
        Action::Tag => tag_inquire(connection),
        Action::Current => {
            let Some(current) = get_current_frame(connection) else {return ExitCode::FAILURE};
            use crate::schema::projects::dsl::*;
            let project = projects
                .filter(id.eq(current.project))
                .load::<Project>(connection)
                .expect("Failed to query database")
                .pop()
                .unwrap_or_else(|| panic!("Found no project for id {}", current.id));

            let task = &project.name;
            println!("{}: {}", task, current.start.elapsed().format());
        }
    }

    ExitCode::SUCCESS
}
