use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use melo::core::config::settings::Settings;
use melo::core::db::bootstrap::DatabaseBootstrap;
use melo::core::error::MeloResult;
use melo::daemon::tasks::RuntimeTaskStore;
use melo::domain::library::metadata::{LyricsSourceKind, MetadataReader, SongMetadata};
use melo::domain::library::service::LibraryService;
use melo::domain::open::service::{OpenRequest, OpenService};
use melo::domain::player::backend::NoopBackend;
use melo::domain::player::service::PlayerService;
use melo::domain::playlist::service::PlaylistService;

struct SlowReader {
    delays: HashMap<String, Duration>,
}

impl MetadataReader for SlowReader {
    fn read(&self, path: &Path) -> MeloResult<SongMetadata> {
        if let Some(delay) = self.delays.get(
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default(),
        ) {
            std::thread::sleep(*delay);
        }

        Ok(SongMetadata {
            title: path.file_stem().unwrap().to_string_lossy().to_string(),
            artist: Some("Aimer".to_string()),
            album: Some("Singles".to_string()),
            track_no: None,
            disc_no: None,
            duration_seconds: Some(180.0),
            genre: None,
            lyrics: None,
            lyrics_source_kind: LyricsSourceKind::None,
            lyrics_format: None,
            embedded_artwork: None,
            format: Some("flac".to_string()),
            bitrate: None,
            sample_rate: None,
            bit_depth: None,
            channels: None,
        })
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn directory_open_returns_after_prewarm_and_background_scan_finishes_later() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("01-first.flac"), b"audio").unwrap();
    std::fs::write(temp.path().join("02-second.flac"), b"audio").unwrap();
    std::fs::write(temp.path().join("03-third.flac"), b"audio").unwrap();

    let mut settings = Settings::for_test(temp.path().join("melo.db"));
    settings.open.max_depth = 1;
    settings.open.prewarm_limit = 1;
    settings.open.background_jobs = 2;
    DatabaseBootstrap::new(&settings).init().await.unwrap();

    let player = Arc::new(PlayerService::new(Arc::new(NoopBackend)));
    player.start_runtime_event_loop();
    player.start_progress_loop();
    let library = LibraryService::new(
        settings.clone(),
        Arc::new(SlowReader {
            delays: HashMap::from([
                ("02-second.flac".to_string(), Duration::from_millis(250)),
                ("03-third.flac".to_string(), Duration::from_millis(250)),
            ]),
        }),
    );
    let playlists = PlaylistService::new(settings.clone());
    let tasks = Arc::new(RuntimeTaskStore::new());
    let open = OpenService::new(
        settings.clone(),
        library,
        playlists,
        Arc::clone(&player),
        Arc::clone(&tasks),
    );

    let started = Instant::now();
    let response = open
        .open(OpenRequest {
            target: temp.path().to_string_lossy().to_string(),
            mode: "path_dir".to_string(),
        })
        .await
        .unwrap();

    assert!(started.elapsed() < Duration::from_millis(200));
    assert_eq!(response.snapshot.queue_len, 1);

    let mut receiver = tasks.subscribe();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if receiver
                .borrow()
                .clone()
                .is_some_and(|task| task.queued_count == 3)
            {
                break;
            }
            receiver.changed().await.unwrap();
        }
    })
    .await
    .unwrap();

    assert_eq!(player.snapshot().await.queue_len, 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn background_scan_appends_remaining_tracks_in_discovery_order() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("01-first.flac"), b"audio").unwrap();
    std::fs::write(temp.path().join("02-second.flac"), b"audio").unwrap();
    std::fs::write(temp.path().join("03-third.flac"), b"audio").unwrap();

    let mut settings = Settings::for_test(temp.path().join("melo.db"));
    settings.open.max_depth = 1;
    settings.open.prewarm_limit = 1;
    settings.open.background_jobs = 2;
    DatabaseBootstrap::new(&settings).init().await.unwrap();

    let player = Arc::new(PlayerService::new(Arc::new(NoopBackend)));
    player.start_runtime_event_loop();
    player.start_progress_loop();
    let library = LibraryService::new(
        settings.clone(),
        Arc::new(SlowReader {
            delays: HashMap::from([
                ("02-second.flac".to_string(), Duration::from_millis(250)),
                ("03-third.flac".to_string(), Duration::from_millis(50)),
            ]),
        }),
    );
    let playlists = PlaylistService::new(settings.clone());
    let tasks = Arc::new(RuntimeTaskStore::new());
    let open = OpenService::new(
        settings.clone(),
        library,
        playlists,
        Arc::clone(&player),
        Arc::clone(&tasks),
    );

    open.open(OpenRequest {
        target: temp.path().to_string_lossy().to_string(),
        mode: "path_dir".to_string(),
    })
    .await
    .unwrap();

    let mut receiver = tasks.subscribe();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if receiver
                .borrow()
                .clone()
                .is_some_and(|task| task.queued_count == 3)
            {
                break;
            }
            receiver.changed().await.unwrap();
        }
    })
    .await
    .unwrap();

    assert_eq!(
        player.snapshot().await.queue_preview,
        vec![
            "01-first".to_string(),
            "02-second".to_string(),
            "03-third".to_string()
        ]
    );
}
