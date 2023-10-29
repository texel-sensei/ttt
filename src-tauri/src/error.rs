use std::fmt::Display;

use serde::{Serialize, Serializer};

use crate::model::Frame;

#[derive(Debug)]
pub enum Error {
    /// Trying to start a new frame, while one is already active.
    AlreadyTracking(Frame),

    /// No frame is currently running
    NoActiveFrame,

    /// Could not find the project with the given name
    ProjectNotFound(String),

    /// Could not find the tag with the given name
    TagNotFound(String),

    DatabaseError(diesel::result::Error),
    DatabaseConnectionError(diesel::prelude::ConnectionError),
    IoError(std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<diesel::result::Error> for Error {
    fn from(error: diesel::result::Error) -> Self {
        Self::DatabaseError(error)
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::IoError(error)
    }
}

impl From<diesel::prelude::ConnectionError> for Error {
    fn from(error: diesel::prelude::ConnectionError) -> Self {
        Self::DatabaseConnectionError(error)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::DatabaseError(e) => write!(f, "Database Error: {}", e),
            Error::IoError(e) => write!(f, "IO Error: {}", e),
            Error::DatabaseConnectionError(e) => write!(f, "Database Connection Error: {}", e),
            Error::AlreadyTracking(frame) => write!(f, "Already tracking a frame: {frame:?}"),
            Error::ProjectNotFound(name) => write!(f, "Project does not exist: {name}"),
            Error::TagNotFound(name) => write!(f, "Tag does not exist: {name}"),
            Error::NoActiveFrame => write!(f, "No active frame"),
        }
    }
}

impl std::error::Error for Error {}

impl Serialize for Error {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> std::result::Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        match self {
            Error::AlreadyTracking(frame) => {
                serializer.serialize_newtype_variant("Error", 0, "AlreadyTracking", frame)
            }
            Error::NoActiveFrame => serializer.serialize_unit_variant("Error", 1, "NoActiveFrame"),
            Error::ProjectNotFound(projectname) => {
                serializer.serialize_newtype_variant("Error", 2, "ProjectNotFound", projectname)
            }
            Error::TagNotFound(tagname) => {
                serializer.serialize_newtype_variant("Error", 3, "TagNotFound", tagname)
            }
            Error::DatabaseError(dberror) => serializer.serialize_newtype_variant(
                "Error",
                4,
                "DatabaseError",
                &dberror.to_string(),
            ),
            Error::DatabaseConnectionError(connectionerror) => serializer
                .serialize_newtype_variant(
                    "Error",
                    5,
                    "DatabaseConnectionError",
                    &connectionerror.to_string(),
                ),
            Error::IoError(ioerror) => {
                serializer.serialize_newtype_variant("Error", 6, "IoError", &ioerror.to_string())
            }
        }
    }
}
