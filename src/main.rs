use std::{error::Error, env, io::{stdin, Read}};

use chrono::NaiveDateTime;
use clap::{Parser, Subcommand};
use diesel::{Queryable, SqliteConnection, Connection, prelude::*};
use dotenvy::dotenv;
use inquire::{DateSelect, Confirm, CustomType, MultiSelect, Select};
mod schema;
use schema::posts;

#[derive(Queryable)]
pub struct Post {
    pub id: i32,
    pub title: String,
    pub body: String,
    pub published: bool,
}

#[derive(Insertable)]
#[diesel(table_name = posts)]
pub struct NewPost<'a> {
    pub title: &'a str,
    pub body: &'a str,
}

pub fn establish_connection() -> SqliteConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    SqliteConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}

pub fn create_post(conn: &mut SqliteConnection, title: &str, body: &str) -> usize {
    let new_post = NewPost { title, body };

    diesel::insert_into(posts::table)
        .values(&new_post)
        .execute(conn)
        .expect("Error saving new post")
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
    println!("What would you like your title to be?");
    let mut titlestr = String::new();
    stdin().read_line(&mut titlestr).unwrap();
    let titlestr = &titlestr[..(titlestr.len() - 1)]; // Drop the newline character
    println!(
        "\nOk! Let's write {} (Press {} when finished)\n",
        titlestr, "CTRL+D"
    );
    let mut mybody = String::new();
    stdin().read_to_string(&mut mybody).unwrap();

    let _ = create_post(connection, titlestr, &mybody);
    println!("\nSaved draft {}", titlestr);

    use self::schema::posts::dsl::*;

    let results = posts
        .filter(published.eq(true))
        .limit(5)
        .load::<Post>(connection)
        .expect("Error loading posts");

    println!("Displaying {} posts", results.len());
    for post in results {
        println!("{}", post.title);
        println!("-----------\n");
        println!("{}", post.body);
    }
    let results = posts
        .filter(published.eq(false))
        .limit(5)
        .load::<Post>(connection)
        .expect("Error loading posts");

    println!("Displaying {} unpublished posts", results.len());
    for post in results {
        println!("{}", post.title);
        println!("-----------\n");
        println!("{}", post.body);
    }
    return;
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
