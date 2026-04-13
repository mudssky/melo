use std::path::Path;

use crate::cli::launch::{DefaultLaunchDecision, choose_default_launch_decision};

fn playing_snapshot() -> crate::core::model::tui::TuiSnapshot {
    crate::core::model::tui::TuiSnapshot {
        player: crate::core::model::player::PlayerSnapshot {
            playback_state: crate::core::model::player::PlaybackState::Playing
                .as_str()
                .to_string(),
            ..crate::core::model::player::PlayerSnapshot::default()
        },
        active_task: None,
        playlist_browser: crate::core::model::tui::PlaylistBrowserSnapshot {
            default_view: crate::core::model::tui::TuiViewKind::Playlist,
            default_selected_playlist: Some("Favorites".to_string()),
            current_playing_playlist: Some(crate::core::model::tui::PlaylistListItem {
                name: "Favorites".to_string(),
                kind: "static".to_string(),
                count: 3,
                is_current_playing_source: true,
                is_ephemeral: false,
            }),
            visible_playlists: Vec::new(),
        },
        current_track: crate::core::model::tui::CurrentTrackSnapshot::default(),
    }
}

#[test]
fn choose_default_launch_decision_preserves_active_playback_session() {
    let decision = choose_default_launch_decision(Path::new("D:/Music/Aimer"), &playing_snapshot());

    assert_eq!(
        decision,
        DefaultLaunchDecision::PreserveCurrentSession {
            launch_cwd: "D:/Music/Aimer".to_string(),
            playlist_name: "Favorites".to_string(),
        }
    );
}

#[test]
fn choose_default_launch_decision_opens_launch_cwd_when_not_playing() {
    let mut snapshot = playing_snapshot();
    snapshot.player.playback_state = crate::core::model::player::PlaybackState::Stopped
        .as_str()
        .to_string();

    let decision = choose_default_launch_decision(Path::new("D:/Music/Aimer"), &snapshot);

    assert_eq!(
        decision,
        DefaultLaunchDecision::OpenLaunchCwd {
            launch_cwd: "D:/Music/Aimer".to_string(),
        }
    );
}
