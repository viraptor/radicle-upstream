use std::convert::TryFrom as _;

use radicle_source::{surf::vcs::git, error};
use radicle_daemon::{signer::BoxedSigner, state, net, librad::git::types::{Reference, Single}};

use crate::error::Error;

/// Provide a repo [`git::Browser`] where the `Browser` is initialised with the provided
/// `reference`.
///
/// See [`find_default_branch`] and [`get_branch`] for obtaining a
/// [`Reference`].
///
/// # Errors
///   * If the namespace of the reference could not be converted to a [`git::Namespace`].
///   * If we could not open the backing storage.
///   * If we could not initialise the `Browser`.
///   * If the callback provided returned an error.
pub async fn with_browser<T, F>(
    peer: &net::peer::Peer<BoxedSigner>,
    reference: Reference<Single>,
    callback: F,
) -> Result<T, Error>
where
    F: FnOnce(&mut git::Browser) -> Result<T, radicle_source::Error> + Send,
{
    let namespace = git::namespace::Namespace::try_from(
        reference
            .namespace
            .ok_or(state::Error::MissingNamespace)?
            .to_string()
            .as_str(),
    ).map_err(error::Error::from)?;

    let branch = match reference.remote {
        None => git::Branch::local(reference.name.as_str()),
        Some(peer) => git::Branch::remote(
            &format!("heads/{}", reference.name.as_str()),
            &peer.to_string(),
        ),
    };

    let monorepo = state::monorepo(peer);
    let repo = git::Repository::new(monorepo).map_err(error::Error::from)?;
    let mut browser =
        git::Browser::new_with_namespace(&repo, &namespace, branch).map_err(error::Error::from)?;

    Ok(callback(&mut browser)?)
}


