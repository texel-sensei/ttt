use std::fmt::Display;

#[derive(Debug)]
pub enum Error {
    DatabaseError (diesel::result::Error),
    DatabaseConnectionError (diesel::prelude::ConnectionError),
    IoError (std::io::Error),
}


pub type Result<T> = std::result::Result<T,Error>;


impl From<diesel::result::Error> for Error {
    fn from(error: diesel::result::Error) -> Self {
        Self::DatabaseError(error)
    }
}

impl From<std::io::Error> for Error{
    fn from(error: std::io::Error) -> Self {
        Self::IoError(error)
    }
}

impl From<diesel::prelude::ConnectionError> for Error{
    fn from(error: diesel::prelude::ConnectionError) -> Self {
        Self::DatabaseConnectionError(error)
    }
}

impl Display for Error{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::DatabaseError(e) => write!(f,"Database Error: {}", e),
            Error::IoError(e) => write!(f,"IO Error: {}", e),
            Error::DatabaseConnectionError(e) => write!(f,"Database Connection Error: {}", e),
        }
    }
}

impl std::error::Error for Error{}
