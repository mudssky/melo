use crate::core::model::player::QueueItem;
use crate::domain::player::queue::PlayerQueue;

fn item(song_id: i64, title: &str) -> QueueItem {
    QueueItem {
        song_id,
        path: format!("D:/Music/{title}.flac"),
        title: title.to_string(),
        duration_seconds: Some(180.0),
    }
}

#[test]
fn queue_insert_before_current_advances_current_index() {
    let mut queue = PlayerQueue::from_items(vec![item(1, "One"), item(2, "Two")], Some(1));

    queue.insert(0, item(3, "Zero")).unwrap();

    assert_eq!(queue.current_index(), Some(2));
    assert_eq!(queue.current().unwrap().title, "Two");
    assert_eq!(queue.len(), 3);
}

#[test]
fn queue_remove_current_prefers_next_item() {
    let mut queue = PlayerQueue::from_items(
        vec![item(1, "One"), item(2, "Two"), item(3, "Three")],
        Some(1),
    );

    let removed = queue.remove(1).unwrap().unwrap();

    assert_eq!(removed.title, "Two");
    assert_eq!(queue.current_index(), Some(1));
    assert_eq!(queue.current().unwrap().title, "Three");
}

#[test]
fn queue_move_current_item_tracks_new_index() {
    let mut queue = PlayerQueue::from_items(
        vec![item(1, "One"), item(2, "Two"), item(3, "Three")],
        Some(0),
    );

    queue.move_item(0, 2).unwrap();

    assert_eq!(queue.current_index(), Some(2));
    assert_eq!(queue.current().unwrap().title, "One");
}

#[test]
fn queue_clear_resets_current_index_and_navigation() {
    let mut queue = PlayerQueue::from_items(vec![item(1, "One"), item(2, "Two")], Some(1));

    queue.clear();

    assert_eq!(queue.current_index(), None);
    assert_eq!(queue.len(), 0);
    assert!(!queue.has_next());
    assert!(!queue.has_prev());
}
