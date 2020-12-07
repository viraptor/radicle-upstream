use std::time::Instant;

use tokio::sync::oneshot;

use librad::{
    identities::Urn,
    net::{
        peer::{Gossip, PeerEvent},
        protocol::ProtocolEvent,
    },
    peer::PeerId,
};

use crate::{
    peer::announcement,
    request::{waiting_room, SomeRequest},
};

/// Significant events that occur during peer’s lifetime.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum Input {
    /// Announcement subroutine lifecycle events.
    Announce(Announce),
    /// Peer state change events.
    Control(Control),
    /// Inputs from the underlying peer API.
    Peer(PeerEvent),
    /// Inputs from the underlying coco protocol.
    Protocol(ProtocolEvent<Gossip>),
    /// Lifecycle events during peer sync operations.
    PeerSync(Sync),
    /// Request subroutine events that wish to attempt to fetch an identity from the network.
    Request(Request),
    /// Scheduled timeouts which can occur.
    Timeout(Timeout),
}

/// Announcement subroutine lifecycle events.
#[derive(Clone, Debug)]
pub enum Announce {
    /// Operation failed.
    Failed,
    /// Operation succeeded and emitted the enclosed list of updates.
    Succeeded(announcement::Updates),
    /// The ticker duration has elapsed.
    Tick,
}

/// Requests from the peer control.
#[derive(Debug)]
pub enum Control {
    /// New status.
    Status(oneshot::Sender<super::Status>),

    /// Cancel an ongoing project search.
    CancelRequest(
        Urn,
        Instant,
        oneshot::Sender<Result<Option<SomeRequest<Instant>>, waiting_room::Error>>,
    ),
    /// Initiate a new project search on the network.
    CreateRequest(
        Urn,
        Instant,
        oneshot::Sender<waiting_room::Created<Instant>>,
    ),
    /// Request a project search.
    GetRequest(Urn, oneshot::Sender<Option<SomeRequest<Instant>>>),
    /// Request the list of project searches.
    ListRequests(oneshot::Sender<Vec<SomeRequest<Instant>>>),
}

/// Request event for projects requested from the network.
#[derive(Debug)]
pub enum Request {
    /// Started cloning the requested urn from a peer.
    Cloning(Urn, PeerId),
    /// Succeeded cloning from the `RadUrl`.
    Cloned(Urn, PeerId),
    /// Failed to clone from the `RadUrl`.
    Failed {
        /// The URN we attempted to clone.
        urn: Urn,
        // The id of the remote peer we attempted to clone from.
        remote_peer: PeerId,
        /// The reason the clone failed.
        reason: String,
    },
    /// Query the network for the `Urn`.
    Queried(Urn),
    /// [`crate::request::waiting_room::WaitingRoom`] query interval.
    Tick,
    /// The request for [`Urn`] timed out.
    TimedOut(Urn),
}

/// Lifecycle events during peer sync operations.
#[derive(Debug)]
pub enum Sync {
    /// A sync has been initiated for `PeerId`.
    Started(PeerId),
    /// A sync has failed for `PeerId`.
    Failed(PeerId),
    /// A sync has succeeded for `PeerId`.
    Succeeded(PeerId),
}

/// Scheduled timeouts which can occur.
#[derive(Debug)]
pub enum Timeout {
    /// Grace period is over signaling that we should go offline, no matter how many syncs have
    /// succeeded.
    SyncPeriod,
}
