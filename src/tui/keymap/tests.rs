use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::event::ActionId;
use crate::tui::keymap::{Keymap, Resolution};

#[test]
fn keymap_matches_single_binding() {
    let mut keymap = Keymap::default();
    let resolution = keymap.resolve_key(
        KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
        Instant::now(),
    );

    assert_eq!(resolution, Resolution::Matched(ActionId::FocusNext));
}

#[test]
fn keymap_waits_for_sequence_prefix() {
    let mut keymap = Keymap::default();
    let now = Instant::now();

    assert_eq!(
        keymap.resolve_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE), now),
        Resolution::Pending
    );
    assert_eq!(
        keymap.resolve_key(
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
            now + Duration::from_millis(100)
        ),
        Resolution::Matched(ActionId::JumpTop)
    );
}
