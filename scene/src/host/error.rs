pub use flo_scene_guest::errors::*;

#[cfg(feature="tokio")]
mod tokio_errors {
    use super::*;
    use tokio::io::{Error, ErrorKind};

    pub trait ConnectionErrorTokioExt {
        fn to_connection_error(self) -> ConnectionError;
    }

    impl ConnectionErrorTokioExt for Error {
        fn to_connection_error(self) -> ConnectionError {
            match self.kind() {
                ErrorKind::NotFound             => ConnectionError::TargetNotAvailable,
                ErrorKind::PermissionDenied     => ConnectionError::TargetPermissionRefused,
                ErrorKind::ConnectionRefused    => ConnectionError::TargetConnectionRefused,
                ErrorKind::ConnectionReset      |
                ErrorKind::BrokenPipe           |
                ErrorKind::ConnectionAborted    => ConnectionError::Cancelled,
                ErrorKind::NotConnected         => ConnectionError::IoError(format!("{}", self)),
                ErrorKind::AddrInUse            |
                ErrorKind::AddrNotAvailable     |
                ErrorKind::AlreadyExists        |
                ErrorKind::WouldBlock           |
                ErrorKind::InvalidInput         |
                ErrorKind::InvalidData          |
                ErrorKind::TimedOut             |
                ErrorKind::WriteZero            |
                ErrorKind::Interrupted          |
                ErrorKind::Unsupported          |
                ErrorKind::UnexpectedEof        |
                ErrorKind::OutOfMemory          |
                ErrorKind::Other                |
                _                               => ConnectionError::IoError(self.to_string()),

            }
        }
    }
}

pub use tokio_errors::*;