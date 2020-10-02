//! Machinery to advance the underlying network protocol and manage auxiliary tasks ensuring
//! prorper state updates.

use std::{
    convert::From,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::{Duration, Instant},
};

use futures::{
    future::{self, FutureExt as _},
    stream::{BoxStream, FuturesUnordered, StreamExt as _},
};
use tokio::{
    sync::{broadcast, mpsc, Barrier},
    task::{JoinError, JoinHandle},
    time::interval,
};

use librad::{
    net::{
        peer::{Gossip, RunLoop},
        protocol::ProtocolEvent,
    },
    peer::PeerId,
    uri::{RadUrl, RadUrn},
};

use crate::{
    request::waiting_room::{self, WaitingRoom},
    shared::Shared,
    state::State,
};

mod announcement;
pub use announcement::Announcement;

mod request;

mod run_state;
pub use run_state::{
    AnnounceConfig, AnnounceEvent, Config as RunConfig, Event, RequestCommand, RequestEvent,
    SyncConfig, SyncEvent, TimeoutEvent,
};
use run_state::{Command, RunState};

mod sync;

/// Upper bound of messages stored in receiver channels.
const RECEIVER_CAPACITY: usize = 128;

/// The period at which we ping the `select!` loop.
const PING_PERIOD: Duration = Duration::from_millis(500);

/// Peer operation errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Failed to build and announce state updates.
    #[error(transparent)]
    Announcement(#[from] announcement::Error),

    /// There was an error in a spawned task.
    #[error("the running peer was either cancelled, or one of its tasks panicked")]
    Spawn(#[source] SpawnAbortableError),

    /// Stop-gap until we get rid of crate level errors.
    // TODO(xla): Remove once we transitioned to per module errors.
    #[error(transparent)]
    Crate(#[from] crate::error::Error),
}

/// Local peer to participate in the radicle code-collaboration network.
pub struct Peer {
    /// Peer [`librad::net::peer::RunLoop`] to advance the network protocol.
    run_loop: RunLoop,
    /// Underlying state that is passed to subroutines.
    state: State,
    /// On-disk storage for caching.
    store: kv::Store,
    /// The waiting room for making requests for identities.
    waiting_room: Shared<WaitingRoom<Instant, Duration>>,
    /// Handle used to broadcast [`Event`].
    subscriber: broadcast::Sender<Event>,
    /// Subroutine config.
    run_config: RunConfig,
}

impl Peer {
    /// Constructs a new [`Peer`].
    #[must_use = "give a peer some love"]
    pub fn new(
        run_loop: RunLoop,
        state: State,
        store: kv::Store,
        waiting_room: Shared<WaitingRoom<Instant, Duration>>,
        run_config: RunConfig,
    ) -> Self {
        let (subscriber, _receiver) = broadcast::channel(RECEIVER_CAPACITY);
        Self {
            run_loop,
            state,
            store,
            waiting_room,
            subscriber,
            run_config,
        }
    }

    /// Subscribe to peer events.
    ///
    /// NB(xla): A caller must call this before the run loop is started, as that consumes the peer.
    /// There is also a configured [`RECEIVER_CAPACITY`], which prevents unbounded queues fron
    /// filling up.
    #[must_use = "eat your events"]
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.subscriber.subscribe()
    }

    /// Start up the internal machinery to advance the underlying protocol, react to significant
    /// events and keep auxiliary tasks running.
    ///
    /// The returned [`Running`] future has similar semantics to [`JoinHandle`]: internally, all
    /// tasks are spawned immediately, and polling the future is only necessary to get notified of
    /// errors. Unlike [`JoinHandle`], however, [`Running`] does not detach the tasks. That is,
    /// if and when [`Running`] is dropped, all tasks are cancelled.
    pub fn into_running(self) -> Running {
        let Self {
            run_loop,
            state,
            store,
            waiting_room,
            subscriber,
            run_config,
        } = self;

        // Rendezvous on a barrier to let the subroutines subscribe for protocol
        // events before it actually starts. As `Protocol::subscribe` is async,
        // we would otherwise need to make `into_running` async as well, which
        // yields the weird requirement to double `.await` it.
        let barrier = Arc::new(Barrier::new(2));
        let subroutines = {
            let barrier = barrier.clone();
            SpawnAbortable::new(async move {
                let protocol_events = state.api.protocol().subscribe().await;
                barrier.wait().await;
                Subroutines::new(
                    state,
                    store,
                    waiting_room,
                    run_config,
                    protocol_events,
                    subscriber,
                )
                .await
            })
        };
        let protocol = SpawnAbortable::new(async move {
            barrier.wait().await;
            run_loop.await
        });

        Running {
            protocol,
            subroutines,
        }
    }
}

/// Future returned by [`Peer::into_running`].
#[must_use = "to the sig hup, don't stop, just drop"]
pub struct Running {
    /// Join and abort handles for the protocol run loop.
    protocol: SpawnAbortable<()>,
    /// The [`Subroutines`] associated with this [`Peer`] instance.
    subroutines: SpawnAbortable<Result<(), SpawnAbortableError>>,
}

impl Drop for Running {
    fn drop(&mut self) {
        log::trace!("`peer::Running` is being dropped");
        self.protocol.abort();
        self.subroutines.abort();
    }
}

impl Future for Running {
    type Output = Result<(), Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let err = match self.protocol.poll_unpin(cx) {
            Poll::Ready(val) => match val {
                Err(e) => Some(Error::Spawn(e)),
                Ok(()) => None,
            },
            Poll::Pending => None,
        };

        if let Some(err) = err {
            log::trace!("run loop error: {:?}", err);
            return Poll::Ready(Err(err));
        }

        match self.subroutines.poll_unpin(cx) {
            Poll::Ready(val) => {
                let val = match val {
                    Err(e) | Ok(Err(e)) => Err(Error::Spawn(e)),
                    Ok(Ok(())) => Ok(()),
                };
                Poll::Ready(val)
            },
            Poll::Pending => Poll::Pending,
        }
    }
}

/// [`SpawnAbortable`] errors.
#[derive(Debug, thiserror::Error)]
pub enum SpawnAbortableError {
    /// The spawned task either panicked, or was cancelled by the runtime.
    #[error(transparent)]
    Join(#[from] JoinError),

    /// The spawned task was aborted by calling [`SpawnAbortable::abort`].
    #[error(transparent)]
    Abort(#[from] future::Aborted),
}

/// A spawned task which can also be aborted by the user.
///
/// Stop-gap until we can abort [`JoinHandle`]s directly:
/// tokio-rs@cbb14a7bb9a13363e1abee8caff2bad1f996c263
#[allow(clippy::missing_docs_in_private_items)]
pub struct SpawnAbortable<T> {
    join_handle: JoinHandle<Result<T, future::Aborted>>,
    abort_handle: future::AbortHandle,
}

impl<T> SpawnAbortable<T> {
    /// Create a new [`SpawnAbortable`] from a [`Future`].
    ///
    /// The supplied [`Future`] will be spawned onto the async executor **immediately**!
    pub fn new<Fut>(fut: Fut) -> Self
    where
        Fut: Future<Output = T> + Send + 'static,
        Fut::Output: Send + 'static,
    {
        let (abort_handle, abort_reg) = future::AbortHandle::new_pair();
        let join_handle = tokio::spawn(future::Abortable::new(fut, abort_reg));

        Self {
            join_handle,
            abort_handle,
        }
    }

    /// Abort this future.
    ///
    /// Subsequent polls will return `SpawnAbortableError::Abort`.
    pub fn abort(&mut self) {
        self.abort_handle.abort()
    }
}

impl<T> Drop for SpawnAbortable<T> {
    fn drop(&mut self) {
        self.abort()
    }
}

impl<T> Future for SpawnAbortable<T> {
    type Output = Result<T, SpawnAbortableError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        match self.join_handle.poll_unpin(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(val) => {
                let val = match val {
                    Err(join) => Err(join.into()),
                    Ok(Err(abort)) => Err(abort.into()),
                    Ok(Ok(t)) => Ok(t),
                };
                Poll::Ready(val)
            },
        }
    }
}

/// Management of "subroutine" tasks.
#[allow(clippy::missing_docs_in_private_items)]
struct Subroutines {
    /// In-flight subroutine tasks.
    pending_tasks: FuturesUnordered<SpawnAbortable<()>>,
    /// Events emitted by subroutine tasks.
    task_events: futures::stream::SelectAll<BoxStream<'static, Event>>,

    state: State,
    store: kv::Store,
    waiting_room: Shared<WaitingRoom<Instant, Duration>>,
    run_state: RunState,

    subscriber: broadcast::Sender<Event>,

    announce_sender: mpsc::Sender<AnnounceEvent>,
    peer_sync_sender: mpsc::Sender<SyncEvent>,
    timeout_sender: mpsc::Sender<TimeoutEvent>,
    request_sender: mpsc::Sender<RequestEvent>,
}

impl Subroutines {
    /// Constructor.
    pub fn new(
        state: State,
        store: kv::Store,
        waiting_room: Shared<WaitingRoom<Instant, Duration>>,
        run_config: RunConfig,
        protocol_events: impl futures::Stream<Item = ProtocolEvent<Gossip>> + Send + 'static,
        subscriber: broadcast::Sender<Event>,
    ) -> Self {
        let announce_timer = interval(run_config.announce.interval);
        let ping_timer = interval(PING_PERIOD);

        let (announce_sender, announcements) = mpsc::channel::<AnnounceEvent>(RECEIVER_CAPACITY);
        let (peer_sync_sender, peer_syncs) = mpsc::channel::<SyncEvent>(RECEIVER_CAPACITY);
        let (timeout_sender, timeouts) = mpsc::channel::<TimeoutEvent>(RECEIVER_CAPACITY);
        let (request_sender, requests) = mpsc::channel::<RequestEvent>(RECEIVER_CAPACITY);

        let request_queries = waiting_room::stream::Queries::new(waiting_room.clone().value);
        let request_clones = waiting_room::stream::Clones::new(waiting_room.clone().value);

        let task_events = {
            let mut coalesced = futures::stream::SelectAll::new();
            coalesced.push(protocol_events.map(Event::Protocol).boxed());
            coalesced.push(
                announce_timer
                    .map(|_| Event::Announce(AnnounceEvent::Tick))
                    .boxed(),
            );
            coalesced.push(ping_timer.map(|_| Event::Ping).boxed());
            coalesced.push(announcements.map(Event::Announce).boxed());
            coalesced.push(peer_syncs.map(Event::PeerSync).boxed());
            coalesced.push(timeouts.map(Event::Timeout).boxed());
            coalesced.push(requests.map(Event::Request).boxed());
            coalesced.push(
                request_queries
                    .map(|urn| Event::Request(RequestEvent::Query(urn)))
                    .boxed(),
            );
            coalesced.push(
                request_clones
                    .map(|url| Event::Request(RequestEvent::Clone(url)))
                    .boxed(),
            );

            coalesced
        };

        let run_state = RunState::from(run_config);

        Self {
            pending_tasks: FuturesUnordered::new(),
            task_events,

            state,
            store,
            waiting_room,
            run_state,

            subscriber,

            announce_sender,
            peer_sync_sender,
            timeout_sender,
            request_sender,
        }
    }
}

impl Drop for Subroutines {
    fn drop(&mut self) {
        for task in self.pending_tasks.iter_mut() {
            task.abort()
        }
    }
}

impl Future for Subroutines {
    type Output = Result<(), SpawnAbortableError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        // Drain the task queue
        loop {
            match self.pending_tasks.poll_next_unpin(cx) {
                Poll::Ready(Some(Err(e))) => {
                    log::warn!("error in spawned subroutine task: {:?}", e);
                    return Poll::Ready(Err(e));
                },
                Poll::Ready(Some(Ok(()))) => continue,
                // Either pending, or FuturesUnordered thinks it's done, but
                // we'll enqueue new tasks below
                Poll::Ready(None) | Poll::Pending => break,
            }
        }

        // Collect any task results, and enqueue new tasks if applicable
        loop {
            match self.task_events.poll_next_unpin(cx) {
                Poll::Ready(Some(event)) => {
                    log::debug!("handling subroutine event: {:?}", event);

                    // Ignore if there are no subscribers
                    self.subscriber.send(event.clone()).ok();

                    for cmd in self.run_state.transition(&event) {
                        let task = match cmd {
                            Command::Announce => SpawnAbortable::new(announce(
                                self.state.clone(),
                                self.store.clone(),
                                self.announce_sender.clone(),
                            )),
                            Command::SyncPeer(peer_id) => SpawnAbortable::new(sync(
                                self.state.clone(),
                                peer_id.clone(),
                                self.peer_sync_sender.clone(),
                            )),
                            Command::StartSyncTimeout(sync_period) => SpawnAbortable::new(
                                start_sync_timeout(sync_period, self.timeout_sender.clone()),
                            ),
                            Command::Request(RequestCommand::Query(urn)) => SpawnAbortable::new(
                                query(urn, self.state.clone(), self.waiting_room.clone()),
                            ),
                            Command::Request(RequestCommand::Found(url)) => {
                                SpawnAbortable::new(found(url, self.waiting_room.clone()))
                            },
                            Command::Request(RequestCommand::Clone(url)) => {
                                SpawnAbortable::new(clone(
                                    url,
                                    self.state.clone(),
                                    self.waiting_room.clone(),
                                    self.request_sender.clone(),
                                ))
                            },
                        };

                        self.pending_tasks.push(task);
                    }
                },

                Poll::Ready(None) => return Poll::Ready(Ok(())),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

/// Announcement subroutine.
async fn announce(state: State, store: kv::Store, mut sender: mpsc::Sender<AnnounceEvent>) {
    match announcement::run(&state, &store).await {
        Ok(updates) => {
            sender.send(AnnounceEvent::Succeeded(updates)).await.ok();
        },
        Err(err) => {
            log::error!("announce error: {:?}", err);
            sender.send(AnnounceEvent::Failed).await.ok();
        },
    }
}

/// Peer syncing subroutine.
async fn sync(state: State, peer_id: PeerId, mut sender: mpsc::Sender<SyncEvent>) {
    sender.send(SyncEvent::Started(peer_id.clone())).await.ok();

    match sync::sync(&state, peer_id.clone()).await {
        Ok(_) => {
            sender
                .send(SyncEvent::Succeeded(peer_id.clone()))
                .await
                .ok();
        },
        Err(err) => {
            log::error!("sync error for {}: {:?}", peer_id, err);
            sender.send(SyncEvent::Failed(peer_id.clone())).await.ok();
        },
    }
}

/// Sync timeout subroutine.
async fn start_sync_timeout(sync_period: Duration, mut sender: mpsc::Sender<TimeoutEvent>) {
    tokio::time::delay_for(sync_period).await;
    sender.send(TimeoutEvent::SyncPeriod).await.ok();
}

/// Query subroutine.
async fn query(urn: RadUrn, state: State, waiting_room: Shared<WaitingRoom<Instant, Duration>>) {
    request::query(urn.clone(), state, waiting_room)
        .await
        .unwrap_or_else(|err| {
            log::warn!(
                "an error occurred for the command 'Query' for the URN '{}':\n{}",
                urn,
                err
            );
        });
}

/// Found subroutine.
async fn found(url: RadUrl, waiting_room: Shared<WaitingRoom<Instant, Duration>>) {
    request::found(url.clone(), waiting_room)
        .await
        .unwrap_or_else(|err| {
            log::warn!(
                "an error occurred for the command 'Found' for the URL '{}':\n{}",
                url,
                err
            );
        });
}

/// Clone subroutine.
async fn clone(
    url: RadUrl,
    state: State,
    waiting_room: Shared<WaitingRoom<Instant, Duration>>,
    mut sender: mpsc::Sender<RequestEvent>,
) {
    match request::clone(url.clone(), state, waiting_room).await {
        Ok(()) => {
            sender.send(RequestEvent::Cloned(url)).await.ok();
        },
        Err(err) => {
            log::warn!(
                "an error occurred for the command 'Clone' for the URL '{}':\n{}",
                url,
                err
            );
            sender
                .send(RequestEvent::Failed {
                    url,
                    reason: err.to_string(),
                })
                .await
                .ok();
        },
    }
}
