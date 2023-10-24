use std::{error::Error, process::ExitCode};

use clap::{arg, Args, Parser, Subcommand};
use database::{ArchivedState, Database};
use inquire::{
    list_option::ListOption, validator::Validation, Confirm, CustomType, CustomUserError,
    DateSelect, MultiSelect, Select,
};

mod database;
pub mod error;
mod model;
mod schema;
mod timespan_parser;

use crate::{
    database::TimeSpan,
    model::{Frame, Timestamp},
};

#[derive(Parser)]
#[clap(author, version)]
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
    Start {
        /// Name of the project to start. If no name is given, interactive mode is used to
        /// determine the project.
        name: Option<String>,
    },

    /// Stop tracking the current activity
    Stop,

    /// Print the current project
    Current,

    /// Add a project
    NewProject { name: String },

    /// Add a tag
    NewTag { name: String },

    /// Tag projects interactively
    Tag {
        project: Option<String>,
        tags: Vec<String>,
    },

    /// Analyze activities performed in a time frame
    Analyze(AnalyzeOptions),

    /// List available projects or tags.
    #[command(subcommand)]
    List(ListAction),
}

#[derive(Args, Debug)]
struct ListArgs {
    /// Whether to include archived objects or not
    #[arg(
        long,
        num_args=0..=1,
        default_value_t = ArchivedState::NotArchived,
        default_missing_value="only-archived",
        value_enum
    )]
    archived: ArchivedState,
}

#[derive(Subcommand, Debug)]
enum ListAction {
    Projects {
        #[arg(long, default_value_t = false)]
        with_tags: bool,

        #[command(flatten)]
        args: ListArgs,
    },
    Tags(ListArgs),
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
        let project = db
            .lookup_project(current.project)
            .expect("Database is broken")
            .unwrap();

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

    let data = db
        .get_frames_in_span(span, ArchivedState::Both)
        .expect("Database is broken");

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

fn min_select_validator(input: &[ListOption<&&String>]) -> Result<Validation, CustomUserError> {
    if input.is_empty() {
        Ok(Validation::Invalid("Select at least one element".into()))
    } else {
        Ok(Validation::Valid)
    }
}

fn tag_projects(database: &mut Database, project_name: &str, tag_names: &[String]) {
    let Some(selected_project) = database
        .lookup_project_by_name(project_name)
        .expect("Database is broken")
    else {
        eprintln!("Project {project_name} seems to be missing from the database. Please add it before using it.");
        std::process::exit(1); // TODO: Change this to ExitCode::FAILURE if casting support is
                               // added.
    };

    if selected_project.archived {
        eprintln!(
            "Project {project_name} is archived. Please unarchive the project before using it."
        );
        std::process::exit(1); // TODO: Change this to ExitCode::FAILURE if casting support is
                               // added.
    }

    let tags: Vec<_> = tag_names.iter().map(|tag| {
        let Some(selected_tag) = database.lookup_tag_by_name(tag).expect("Database is broken") else {
            eprintln!("Tag {tag} seems to be missing from the database. Please add it before using it.");
            std::process::exit(1); // TODO: Change this to ExitCode::FAILURE if casting support is
                                   // added.
        };

        if selected_tag.archived {
            eprintln!("Tag {tag} is archived. Please unarchive the tag before using it.");
            std::process::exit(1); // TODO: Change this to ExitCode::FAILURE if casting support is
                                   // added.
        }
        selected_tag

    }).collect();

    database
        .tag_projects(tags, vec![selected_project])
        .expect("Could not tag projects.");
}

fn tag_project_inquire(database: &mut Database, project: &str) {
    let Some(selected_project) = database
        .lookup_project_by_name(project)
        .expect("Database is broken")
    else {
        eprintln!("Project {project} seems to be missing from the database. Please add it before using it.");
        std::process::exit(1); // TODO: Change this to ExitCode::FAILURE if casting support is
                               // added.
    };

    if selected_project.archived {
        eprintln!("Project {project} is archived. Please unarchive the project before using it.");
        std::process::exit(1); // TODO: Change this to ExitCode::FAILURE if casting support is
                               // added.
    }

    let mut possible_tags = database
        .all_tags(ArchivedState::NotArchived)
        .expect("Database is broken");
    if possible_tags.is_empty() {
        println!("Please create a tag before tagging.");
        return;
    }

    let selected_tags: Vec<_> = MultiSelect::new(
        "Select the tags to apply to selected projects.",
        possible_tags.iter().map(|p| &p.name).collect(),
    )
    .with_validator(min_select_validator)
    .raw_prompt()
    .unwrap()
    .into_iter()
    .map(|item| item.index)
    .collect();

    database
        .tag_projects(
            pick(&mut possible_tags, &selected_tags),
            vec![selected_project],
        )
        .expect("Could not tag projects.");
}

fn tag_inquire(database: &mut Database) {
    let mut possible_projects = database
        .all_projects(ArchivedState::NotArchived)
        .expect("Database is broken");
    if possible_projects.is_empty() {
        println!("Please create a project before tagging.");
        return;
    }

    let mut possible_tags = database
        .all_tags(ArchivedState::NotArchived)
        .expect("Database is broken");
    if possible_tags.is_empty() {
        println!("Please create a tag before tagging.");
        return;
    }

    let selected_projects: Vec<_> = MultiSelect::new(
        "Select the projects to tag",
        possible_projects.iter().map(|p| &p.name).collect(),
    )
    .with_validator(min_select_validator)
    .raw_prompt()
    .unwrap()
    .into_iter()
    .map(|item| item.index)
    .collect();

    let selected_tags: Vec<_> = MultiSelect::new(
        "Select the tags to apply to selected projects.",
        possible_tags.iter().map(|p| &p.name).collect(),
    )
    .with_validator(min_select_validator)
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
        Action::Start { name } => {
            let mut project = match name {
                Some(name) => {
                    let Some(selected) = database
                        .lookup_project_by_name(&name)
                        .expect("Error querying the database.")
                    else {
                        eprintln!("Project {name} does not exist in this timeline ;)");
                        return ExitCode::FAILURE;
                    };
                    if selected.archived {
                        eprintln!("Project {name} is archived. Please remove the archived flag.");
                        return ExitCode::FAILURE;
                    }
                    selected
                }
                None => {
                    let possible_projects = database
                        .all_projects(ArchivedState::NotArchived)
                        .expect("Database is broken");
                    if possible_projects.is_empty() {
                        println!("Please create a project before starting a task.");
                        return ExitCode::FAILURE;
                    }
                    let selected_project = Select::new(
                        "Select the project to start",
                        possible_projects.iter().map(|p| &p.name).collect(),
                    )
                    .raw_prompt();

                    use inquire::InquireError::*;
                    let selected_project = match selected_project {
                        Ok(t) => t,
                        Err(OperationCanceled | OperationInterrupted) => return ExitCode::SUCCESS,
                        Err(err) => panic!("Failed to inquire project: {err}"),
                    };

                    let index = selected_project.index;
                    possible_projects[index].clone()
                }
            };

            let _ = stop_current_frame(&mut database);

            database
                .start(&mut project)
                .expect("Failed to start project");
            println!("Started project {}", project.name);
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
            println!("Created project {name}");
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
            println!("Created tag {name}");
        }
        Action::Tag { project, tags } => match (project, AsRef::<[String]>::as_ref(&tags)) {
            (None, []) => tag_inquire(&mut database),
            (Some(project), []) => tag_project_inquire(&mut database, &project),
            (Some(project), tags) => tag_projects(&mut database, &project, tags),
            (None, _) => unreachable!(),
        },
        Action::Current => {
            let Ok(current) = database.current_frame() else {
                return ExitCode::FAILURE;
            };
            let project = database
                .lookup_project(current.project)
                .expect("Database is broken")
                .unwrap_or_else(|| panic!("Found no project for id {}", current.id()));

            let task = &project.name;
            println!("{}: {}", task, current.start.elapsed().format());
        }
        Action::List(action) => list(&mut database, action).expect("Database is broken"),
    }

    ExitCode::SUCCESS
}

fn list(db: &mut Database, action: ListAction) -> crate::error::Result<()> {
    let to_print: Vec<_> = match action {
        ListAction::Projects { args, with_tags } => db
            .all_projects(args.archived)?
            .into_iter()
            .map(|p| {
                if with_tags {
                    let tags = db
                        .lookup_tags_for_project(p.id())
                        .expect("Database is broken");
                    let tags: Vec<_> = tags.into_iter().map(|t| format!("+{}", t.name)).collect();
                    let tags = tags.join(" ");
                    if tags.is_empty() {
                        p.name
                    } else {
                        format!("{} {}", p.name, tags)
                    }
                } else {
                    p.name
                }
            })
            .collect(),
        ListAction::Tags(args) => db
            .all_tags(args.archived)?
            .into_iter()
            .map(|t| t.name)
            .collect(),
    };

    for item in to_print {
        println!("{item}");
    }

    Ok(())
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
