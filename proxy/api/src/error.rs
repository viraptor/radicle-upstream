//! Proxy library errors usable for caller control flow and additional context for API responses.

use std::io;

use radicle_daemon::{keystore, state, peer, Urn, request};

/// All error variants the API will return.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Keystore error.
    #[error(transparent)]
    Keystore(#[from] keystore::Error),

    /// Error interacting with [`radicle_daemon::net::peer::Peer`].
    #[error(transparent)]
    State(#[from] state::Error),

    #[error(transparent)]
    Source(#[from] radicle_source::error::Error),

    #[error(transparent)]
    Peer(#[from] peer::Error),

    /// An I/O error occurred.
    #[error(transparent)]
    Io(#[from] io::Error),

    #[error("the current session is in use by `{0}`")]
    SessionInUse(Urn),

    /// Issues when access persistent storage.
    #[error(transparent)]
    Store(#[from] kv::Error),

    /// The key store is sealed
    #[error("keystore is sealed")]
    KeystoreSealed,

    /// The passphrase was incorrect
    #[error("passphrase incorrect")]
    WrongPassphrase,

    /// The request auth token differs from the one in context
    #[error("invalid authentication token")]
    InvalidAuthCookie,

    /// Errors stemming from [`request::waiting_room::WaitingRoom`] interactions.
    #[error(transparent)]
    WaitingRoom(#[from] request::waiting_room::Error),

    #[error("project not found")]
    ProjectNotFound,

    #[error("missing default branch")]
    MissingDefaultBranch,
}
