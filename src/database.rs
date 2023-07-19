use clap::ValueEnum;
use diesel::{prelude::*, SqliteConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations};
use directories::ProjectDirs;
use dotenvy::dotenv;
use itertools::iproduct;
use std::{env, fs::create_dir_all};

use crate::{
    error::{Error, Result},
    model::{Frame, NewFrame, NewProject, NewTag, Project, Tag, TagProject, Timestamp},
    schema::{frames, projects, tags, tags_per_project},
};

macro_rules! query_table {
    ($database:expr, $table:ident, $type:ty, $include_archived:expr) => {{
        use crate::schema::$table::dsl::*;

        use ArchivedState::*;
        match $include_archived {
            state @ (NotArchived | OnlyArchived) => $table
                .filter(archived.eq(matches!(state, OnlyArchived)))
                .order_by(last_access_time)
                .load::<$type>($database),
            Both => $table.order_by(last_access_time).load::<$type>($database),
        }
    }};
}

pub struct Database {
    connection: SqliteConnection,
}

impl Database {
    pub fn new() -> Result<Self> {
        let connection = establish_connection()?;
        Ok(Self { connection })
    }

    pub fn current_frame(&mut self) -> Result<Frame> {
        use crate::schema::frames::dsl::*;
        let mut current = frames
            .filter(end.is_null())
            .load::<Frame>(&mut self.connection)?;
        current.pop().ok_or(Error::NoActiveFrame)
    }

    /// Start a new frame for the given project.
    pub fn start(&mut self, project: &mut Project) -> Result<Frame> {
        if let Ok(existing) = self.current_frame() {
            return Err(Error::AlreadyTracking(existing));
        }

        let now = Timestamp::now();
        let frame = NewFrame {
            project: project.id(),
            start: &now,
            end: None,
        };
        self.connection.transaction(|con| {
            Self::write_projects_impl(con, std::iter::once(project))?;
            Ok(diesel::insert_into(frames::table)
                .values(&frame)
                .get_result(con)?)
        })
    }

    /// Stop the currently running frame, if any.
    /// In case no frame is currently active this acts as a no-op.
    ///
    /// Returns the stopped frame if it was stopped or None in case no frame was active.
    ///
    /// ```no_run
    /// # use ttt::database::Database;
    /// let mut db = Database::new().unwrap();
    /// assert!(db.stop().unwrap().is_none());
    /// ```
    pub fn stop(&mut self) -> Result<Option<Frame>> {
        let mut frame = match self.current_frame() {
            Ok(frame) => frame,
            Err(Error::NoActiveFrame) => return Ok(None),
            Err(e) => return Err(e),
        };

        let now = Timestamp::now();
        frame.end = Some(now);
        self.update_frame(&frame)?;

        Ok(Some(frame))
    }

    /// Search the project for the given id. Return None if no project belongs to that id.
    pub fn lookup_project(&mut self, project_id: i32) -> Result<Option<Project>> {
        use crate::schema::projects::dsl::*;
        Ok(projects
            .filter(id.eq(project_id))
            .load::<Project>(&mut self.connection)?
            .pop())
    }

    /// Return list of all projects sorted by their last access time.
    pub fn all_projects(&mut self, include_archived: ArchivedState) -> Result<Vec<Project>> {
        Ok(query_table!(
            &mut self.connection,
            projects,
            Project,
            include_archived
        )?)
    }

    /// Return list of all tags sorted by their last access time.
    pub fn all_tags(&mut self, include_archived: ArchivedState) -> Result<Vec<Tag>> {
        Ok(query_table!(
            &mut self.connection,
            tags,
            Tag,
            include_archived
        )?)
    }

    /// Return list of all frames, sorted by their starting date.
    #[allow(dead_code)]
    pub fn all_frames(&mut self, include_archived: ArchivedState) -> Result<Vec<Frame>> {
        match include_archived {
            state @ (ArchivedState::NotArchived | ArchivedState::OnlyArchived) => {
                Ok(projects::table
                    .inner_join(frames::table)
                    .select(frames::all_columns)
                    .filter(projects::archived.eq(matches!(state, ArchivedState::OnlyArchived)))
                    .order_by(frames::start)
                    .load::<Frame>(&mut self.connection)?)
            }

            ArchivedState::Both => Ok(frames::table
                .order_by(frames::start)
                .load::<Frame>(&mut self.connection)?),
        }
    }

    pub fn get_frames_in_span(
        &mut self,
        (start, end): TimeSpan,
        include_archived: ArchivedState,
    ) -> Result<Vec<(Project, Frame)>> {
        // TODO(texel, 2022-09-29): Remove this assert once the TimeSpan type guarantees that fact
        assert!(start < end);

        match include_archived {
            state @ (ArchivedState::NotArchived | ArchivedState::OnlyArchived) => {
                Ok(projects::table
                    .inner_join(frames::table)
                    .select((projects::all_columns, frames::all_columns))
                    .filter(projects::archived.eq(matches!(state, ArchivedState::OnlyArchived)))
                    .filter(frames::end.ge(start))
                    .or_filter(frames::end.is_null())
                    .filter(frames::start.lt(end))
                    .order_by(frames::start)
                    .load::<(Project, Frame)>(&mut self.connection)?)
            }

            ArchivedState::Both => Ok(frames::table
                .inner_join(projects::table)
                .select((projects::all_columns, frames::all_columns))
                .filter(frames::end.ge(start))
                .or_filter(frames::end.is_null())
                .filter(frames::start.lt(end))
                .order_by(frames::start)
                .load::<(Project, Frame)>(&mut self.connection)?),
        }
    }

    /// Write the given projects into the database.
    #[allow(dead_code)]
    pub fn write_projects<'a>(
        &mut self,
        items: impl IntoIterator<Item = &'a mut Project>,
    ) -> Result<()> {
        Self::write_projects_impl(&mut self.connection, items)
    }

    fn write_projects_impl<'a>(
        connection: &mut SqliteConnection,
        items: impl IntoIterator<Item = &'a mut Project>,
    ) -> Result<()> {
        connection.transaction(|connection| {
            use crate::schema::projects::dsl::*;
            let now = Timestamp::now();
            for item in items {
                item.last_access_time = now;
                diesel::insert_into(projects)
                    .values(&*item)
                    .on_conflict(id)
                    .do_update()
                    .set(&*item)
                    .execute(connection)?;
            }
            Ok(())
        })
    }

    /// Create a new tag and return it.
    pub fn create_tag(&mut self, name: impl AsRef<str>) -> Result<Tag> {
        let new_tag = NewTag {
            name: name.as_ref(),
            last_access_time: &Timestamp::now(),
        };
        Ok(diesel::insert_into(tags::table)
            .values(&new_tag)
            .get_result(&mut self.connection)?)
    }

    /// Create a new project and return it.
    pub fn create_project(&mut self, name: impl AsRef<str>) -> Result<Project> {
        let new_project = NewProject {
            name: name.as_ref(),
            last_access_time: &Timestamp::now(),
        };
        Ok(diesel::insert_into(projects::table)
            .values(&new_project)
            .get_result(&mut self.connection)?)
    }

    /// Write the given tags to the database.
    /// This function acts as a transaction, the database is only modified if all tags can be
    /// written successfully.
    #[allow(dead_code)]
    pub fn write_tags<'a>(&mut self, tags: impl IntoIterator<Item = &'a mut Tag>) -> Result<()> {
        Self::write_tags_impl(&mut self.connection, tags)
    }

    fn write_tags_impl<'a>(
        connection: &mut SqliteConnection,
        items: impl IntoIterator<Item = &'a mut Tag>,
    ) -> Result<()> {
        connection.transaction(|connection| {
            use crate::schema::tags::dsl::*;
            let now = Timestamp::now();
            for item in items {
                item.last_access_time = now;
                diesel::insert_into(tags)
                    .values(&*item)
                    .on_conflict(id)
                    .do_update()
                    .set(&*item)
                    .execute(connection)?;
            }
            Ok(())
        })
    }

    pub fn tag_projects(&mut self, mut tags: Vec<Tag>, mut projects: Vec<Project>) -> Result<()> {
        let combination: Vec<_> = iproduct!(&projects, &tags)
            .map(|(p, t)| TagProject {
                project_id: p.id(),
                tag_id: t.id(),
            })
            .collect();

        self.connection.transaction(|connection| {
            diesel::insert_or_ignore_into(tags_per_project::table)
                .values(combination)
                .execute(connection)?;
            Self::write_projects_impl(connection, &mut projects)?;
            Self::write_tags_impl(connection, &mut tags)?;
            Ok(())
        })
    }

    /// Write the given frame back into the database and update the access time of the
    /// corresponding project.
    fn update_frame(&mut self, frame: &Frame) -> Result<()> {
        diesel::update(frame)
            .set(frame)
            .execute(&mut self.connection)?;
        let mut project = self
            .lookup_project(frame.project)?
            .unwrap_or_else(|| panic!("Found no project for id {}", frame.id()));
        project.last_access_time = Timestamp::now();
        diesel::update(&project)
            .set(&project)
            .execute(&mut self.connection)?;

        Ok(())
    }

    /// Search the database for a project with the given name.
    /// This function also returns archived projects.
    pub fn lookup_project_by_name(&mut self, name: &str) -> Result<Option<Project>> {
        Ok(projects::table
            .filter(projects::name.eq(name))
            .get_result(&mut self.connection)
            .optional()?)
    }

    /// Get all tags associated to the given project.
    pub fn lookup_tags_for_project(&mut self, project_id: i32) -> Result<Vec<Tag>> {
        Ok(tags::table
            .inner_join(tags_per_project::table)
            .filter(tags_per_project::project_id.eq(project_id))
            .select(tags::all_columns)
            .get_results(&mut self.connection)?)
    }

    pub fn lookup_tag_by_name(&mut self, name: &str) -> Result<Option<Tag>> {
        Ok(tags::table
            .filter(tags::name.eq(name))
            .get_result(&mut self.connection)
            .optional()?)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ArchivedState {
    NotArchived,
    OnlyArchived,
    Both,
}

pub type TimeSpan = (crate::model::Timestamp, crate::model::Timestamp);

const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

pub fn establish_connection() -> Result<SqliteConnection> {
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

    let mut connection = SqliteConnection::establish(&database_url)?;

    use diesel_migrations::MigrationHarness;
    connection.run_pending_migrations(MIGRATIONS).unwrap();

    Ok(connection)
}
