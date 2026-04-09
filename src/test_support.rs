use crate::core::config::settings::Settings;
use crate::core::db::bootstrap::DatabaseBootstrap;
use crate::domain::library::service::LibraryService;
use crate::domain::playlist::service::PlaylistService;

/// 集成测试辅助器，统一创建临时数据库和配置文件。
pub struct TestHarness {
    /// 临时根目录。
    pub root: tempfile::TempDir,
    /// 测试配置。
    pub settings: Settings,
}

impl TestHarness {
    /// 创建新的测试辅助器。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Self`：测试辅助器
    pub async fn new() -> Self {
        let root = tempfile::tempdir().expect("必须能创建临时目录");
        let settings = Settings::for_test(root.path().join("melo.db"));
        DatabaseBootstrap::new(&settings)
            .init()
            .await
            .expect("必须能初始化测试数据库");

        Self { root, settings }
    }

    /// 写入测试配置文件，并自动附带数据库路径。
    ///
    /// # 参数
    /// - `contents`：附加的 TOML 内容
    ///
    /// # 返回值
    /// - 无
    pub async fn write_config(&self, contents: &str) {
        let config_path = self.root.path().join("config.toml");
        let organize_base_dir = self.root.path().to_string_lossy().replace('\\', "/");
        let contents = contents.replace(
            r#"base_dir = "D:/Library""#,
            &format!(r#"base_dir = "{organize_base_dir}""#),
        );
        let full_contents = format!(
            "[database]\npath = {:?}\n\n{}",
            self.settings.database.path.as_str(),
            contents.trim()
        );
        std::fs::write(&config_path, full_contents).expect("必须能写入测试配置");
        unsafe {
            std::env::set_var("MELO_CONFIG", &config_path);
        }
    }

    /// 创建歌单服务。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `PlaylistService`：测试用歌单服务
    pub fn playlist_service(&self) -> PlaylistService {
        PlaylistService::new(self.settings.clone())
    }

    /// 创建媒体库服务。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `LibraryService`：测试用媒体库服务
    pub fn library_service(&self) -> LibraryService {
        LibraryService::for_test(self.settings.clone())
    }

    /// 向测试数据库直接插入一首歌曲。
    ///
    /// # 参数
    /// - `title`：标题
    /// - `artist`：艺术家
    /// - `album`：专辑
    /// - `year`：年份
    ///
    /// # 返回值
    /// - 无
    pub async fn seed_song(&self, title: &str, artist: &str, album: &str, year: i32) {
        let conn = rusqlite::Connection::open(self.settings.database.path.as_std_path())
            .expect("必须能连接测试数据库");

        conn.execute(
            "INSERT INTO artists (name, sort_name, search_name, created_at, updated_at) VALUES (?1, ?1, lower(?1), datetime('now'), datetime('now'))",
            [artist],
        )
        .expect("必须能插入 artist");
        let artist_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO albums (title, album_artist_id, year, source_dir, created_at, updated_at) VALUES (?1, ?2, ?3, NULL, datetime('now'), datetime('now'))",
            rusqlite::params![album, artist_id, year],
        )
        .expect("必须能插入 album");
        let album_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO songs (path, title, artist_id, album_id, track_no, disc_no, duration_seconds, genre, lyrics, lyrics_source_kind, lyrics_source_path, lyrics_format, lyrics_updated_at, format, bitrate, sample_rate, bit_depth, channels, file_size, file_mtime, added_at, scanned_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 1, 1, 180.0, NULL, NULL, 'none', NULL, NULL, NULL, 'flac', NULL, NULL, NULL, NULL, 0, 0, datetime('now'), datetime('now'), datetime('now'))",
            rusqlite::params![format!("D:/Music/{title}.flac"), title, artist_id, album_id],
        )
        .expect("必须能插入 song");
    }

    /// 在临时目录写入模拟音频文件。
    ///
    /// # 参数
    /// - `relative`：相对临时目录的路径
    ///
    /// # 返回值
    /// - `PathBuf`：实际写入路径
    pub async fn write_song_file(&self, relative: &str) -> std::path::PathBuf {
        let path = self.root.path().join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("必须能创建父目录");
        }
        std::fs::write(&path, b"fake-audio").expect("必须能写入模拟音频");
        path
    }

    /// 在临时目录写入任意文件。
    ///
    /// # 参数
    /// - `relative`：相对路径
    /// - `contents`：文件内容
    ///
    /// # 返回值
    /// - 无
    pub async fn write_file(&self, relative: &str, contents: impl AsRef<[u8]>) {
        let path = self.root.path().join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("必须能创建父目录");
        }
        std::fs::write(path, contents).expect("必须能写入文件");
    }

    /// 判断临时目录下的相对路径是否存在。
    ///
    /// # 参数
    /// - `relative`：相对路径
    ///
    /// # 返回值
    /// - `bool`：是否存在
    pub async fn file_exists(&self, relative: &str) -> bool {
        self.root.path().join(relative).exists()
    }

    /// 插入一首已关联歌词 sidecar 和静态歌单的歌曲。
    ///
    /// # 参数
    /// - `title`：标题
    /// - `artist`：艺术家
    /// - `playlist`：静态歌单名
    /// - `path`：真实文件路径
    ///
    /// # 返回值
    /// - 无
    pub async fn seed_scanned_song_with_sidecar(
        &self,
        title: &str,
        artist: &str,
        playlist: &str,
        path: &std::path::Path,
    ) {
        self.seed_song(title, artist, title, 0).await;
        let conn = rusqlite::Connection::open(self.settings.database.path.as_std_path())
            .expect("必须能连接测试数据库");

        let song_id: i64 = conn
            .query_row("SELECT id FROM songs ORDER BY id DESC LIMIT 1", [], |row| {
                row.get(0)
            })
            .expect("必须能读取 song id");
        conn.execute(
            "UPDATE songs SET path = ?1, lyrics_source_kind = 'sidecar', lyrics_source_path = ?2, lyrics = 'fly high' WHERE id = ?3",
            rusqlite::params![
                path.to_string_lossy().to_string(),
                path.with_extension("lrc").to_string_lossy().to_string(),
                song_id
            ],
        )
        .expect("必须能更新 song 路径");

        conn.execute(
            "INSERT INTO playlists (name, description, created_at, updated_at) VALUES (?1, NULL, datetime('now'), datetime('now'))",
            [playlist],
        )
        .expect("必须能插入静态歌单");
        let playlist_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO playlist_entries (playlist_id, song_id, position, added_at) VALUES (?1, ?2, 0, datetime('now'))",
            rusqlite::params![playlist_id, song_id],
        )
        .expect("必须能关联歌单成员");
    }
}
