use std::error::Error;

use clap::{Parser, Subcommand};
use inquire::{DateSelect, Confirm, CustomType, MultiSelect, Select};


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
    let cli = Cli::parse();
    match cli.action {
        Action::Start => {
            let options = (1i32..5i32).map(|i| i.to_string());

            let answers = Select::new("Select some numbers", options.collect()).prompt().unwrap();
            dbg!(answers);
        },
        Action::Stop => todo!(),
        Action::Analyze ( options ) => {
            if options.is_interactive() {
                do_inquire_stuff().unwrap();
            } else {
                println!("No activities since yesterday, since we didn't implement tracking yet!");
            }
        },
    }
}
