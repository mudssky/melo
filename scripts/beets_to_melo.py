import sqlite3
from pathlib import Path


def migrate(beets_path: str, melo_path: str) -> None:
    """将精简版 beets 数据库迁移到 Melo 数据库。

    参数:
    - beets_path: 源 beets SQLite 数据库路径
    - melo_path: 目标 Melo SQLite 数据库路径

    返回:
    - None
    """
    src = sqlite3.connect(beets_path)
    dst = sqlite3.connect(melo_path)

    dst.executescript(
        """
        CREATE TABLE IF NOT EXISTS artists (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            sort_name TEXT,
            search_name TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS albums (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            album_artist_id INTEGER,
            year INTEGER,
            source_dir TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS songs (
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            title TEXT NOT NULL,
            artist_id INTEGER,
            album_id INTEGER,
            track_no INTEGER,
            disc_no INTEGER,
            duration_seconds REAL,
            genre TEXT,
            lyrics TEXT,
            lyrics_source_kind TEXT NOT NULL DEFAULT 'embedded',
            lyrics_source_path TEXT,
            lyrics_format TEXT,
            lyrics_updated_at TEXT,
            format TEXT,
            bitrate INTEGER,
            sample_rate INTEGER,
            bit_depth INTEGER,
            channels INTEGER,
            file_size INTEGER NOT NULL DEFAULT 0,
            file_mtime INTEGER NOT NULL DEFAULT 0,
            added_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            scanned_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            organized_at TEXT,
            last_organize_rule TEXT,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        """
    )

    artist_map: dict[str, int] = {}
    album_map: dict[int, int] = {}

    for (artist,) in src.execute(
        "SELECT DISTINCT artist FROM items WHERE artist IS NOT NULL AND artist != ''"
    ):
        dst.execute(
            "INSERT INTO artists (name, sort_name, search_name) VALUES (?, ?, ?)",
            (artist, artist, artist.lower()),
        )
        artist_map[artist] = dst.execute("SELECT last_insert_rowid()").fetchone()[0]

    for album_id, title, year, artpath in src.execute(
        "SELECT id, album, year, artpath FROM albums"
    ):
        source_dir = str(Path(artpath).parent) if artpath else None
        dst.execute(
            "INSERT INTO albums (title, year, source_dir) VALUES (?, ?, ?)",
            (title, year, source_dir),
        )
        album_map[album_id] = dst.execute("SELECT last_insert_rowid()").fetchone()[0]

    for row in src.execute(
        "SELECT path, album_id, title, artist, track, disc, length, genre, lyrics, format, bitrate, samplerate, bitdepth, channels FROM items"
    ):
        (
            path,
            album_id,
            title,
            artist,
            track,
            disc,
            length,
            genre,
            lyrics,
            fmt,
            bitrate,
            samplerate,
            bitdepth,
            channels,
        ) = row
        dst.execute(
            """
            INSERT INTO songs (
                path, title, artist_id, album_id, track_no, disc_no, duration_seconds,
                genre, lyrics, format, bitrate, sample_rate, bit_depth, channels
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                path,
                title,
                artist_map.get(artist),
                album_map.get(album_id),
                track,
                disc,
                length,
                genre,
                lyrics,
                fmt,
                bitrate,
                samplerate,
                bitdepth,
                channels,
            ),
        )

    dst.commit()
    src.close()
    dst.close()
