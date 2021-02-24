use nonempty::NonEmpty;
use pretty_assertions::assert_eq;

use coco::{state, RunConfig};

mod common;
use common::{build_peer, init_logging, shia_le_pathbuf};

#[allow(clippy::needless_collect)]
#[tokio::test]
async fn can_browse_peers_branch() -> Result<(), Box<dyn std::error::Error + 'static>> {
    init_logging();

    let alice_tmp_dir = tempfile::tempdir()?;
    let alice_repo_path = alice_tmp_dir.path().join("radicle");
    let alice_peer = build_peer(&alice_tmp_dir, RunConfig::default()).await?;
    let alice = state::init_owner(&alice_peer.peer, "alice".to_string()).await?;

    let bob_tmp_dir = tempfile::tempdir()?;
    let bob_peer = build_peer(&bob_tmp_dir, RunConfig::default()).await?;
    let _bob = state::init_owner(&bob_peer.peer, "bob".to_string()).await?;

    let (alice_peer, alice_addrs) = {
        let peer = alice_peer.peer.clone();
        let listen_addrs = alice_peer.listen_addrs.clone();
        tokio::task::spawn(alice_peer.into_running());
        (peer, listen_addrs)
    };
    let bob_peer = {
        let peer = bob_peer.peer.clone();
        tokio::task::spawn(bob_peer.into_running());
        peer
    };

    let project =
        state::init_project(&alice_peer, &alice, shia_le_pathbuf(alice_repo_path)).await?;

    {
        let alice_peer_id = alice_peer.peer_id();
        state::clone_project(
            &bob_peer,
            project.urn(),
            alice_peer_id,
            alice_addrs.into_iter(),
            None,
        )
        .await?
    };

    let peers = state::list_project_peers(&bob_peer, project.urn()).await?;

    let branch = state::find_default_branch(&bob_peer, project.urn()).await?;
    let revisions = state::with_browser(&bob_peer, branch, |browser| {
        peers
            .into_iter()
            .filter_map(coco::project::Peer::replicated)
            .filter_map(|peer| coco::source::revisions(browser, peer).transpose())
            .collect::<Result<Vec<_>, _>>()
    })
    .await?;

    let expected = coco::source::Revisions {
        peer_id: alice_peer.peer_id(),
        user: alice.into_inner().into_inner(),
        branches: NonEmpty::new(coco::source::Branch::from("it".to_string())),
        tags: vec![],
    };
    assert_eq!(revisions, vec![expected],);

    Ok(())
}
