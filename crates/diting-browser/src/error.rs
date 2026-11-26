use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("navigation error: {0}")]
    Navigation(String),

    /// A navigation that failed in transit, with how. Reads exactly as
    /// [`Error::Navigation`] does.
    #[error("navigation error: {message}")]
    NavigationFailed {
        kind: diting::diting_net::NetFailure,
        message: String,
    },

    #[error("timeout: {0}")]
    Timeout(String),

    #[error("element not found: {0}")]
    ElementNotFound(String),

    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl Error {
    /// How the request behind a failed navigation failed, where that is
    /// known.
    pub fn failure(&self) -> diting::diting_net::NetFailure {
        match self {
            Error::NavigationFailed { kind, .. } => kind.clone(),
            _ => diting::diting_net::NetFailure::Other,
        }
    }
}

/// A page-level navigation error, as this crate's [`Error`]: the failure
/// kind goes with it.
pub(crate) fn navigation(e: diting::diting_browser::page::PageError) -> Error {
    match e.failure() {
        diting::diting_net::NetFailure::Other => Error::Navigation(e.to_string()),
        kind => Error::NavigationFailed {
            kind,
            message: e.to_string(),
        },
    }
}
