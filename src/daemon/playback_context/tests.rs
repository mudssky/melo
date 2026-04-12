use crate::daemon::playback_context::{PlayingPlaylistContext, PlayingPlaylistStore};

#[test]
fn playback_context_store_sets_and_clears_current_playlist() {
    let store = PlayingPlaylistStore::default();
    assert!(store.current().is_none());

    store.set(PlayingPlaylistContext {
        name: "C:/Music/Aimer".to_string(),
        kind: "ephemeral".to_string(),
    });
    assert_eq!(store.current().unwrap().name, "C:/Music/Aimer");

    store.clear();
    assert!(store.current().is_none());
}
