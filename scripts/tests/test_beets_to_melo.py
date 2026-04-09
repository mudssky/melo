import sqlite3
import tempfile
import unittest
from pathlib import Path

from scripts.beets_to_melo import migrate


class BeetsToMeloTests(unittest.TestCase):
    def test_migrate_song_and_album_fields(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            beets_db = temp / "beets.db"
            melo_db = temp / "melo.db"

            conn = sqlite3.connect(beets_db)
            conn.executescript(
                """
                CREATE TABLE albums (id INTEGER PRIMARY KEY, album TEXT, year INTEGER, artpath TEXT);
                CREATE TABLE items (
                    id INTEGER PRIMARY KEY,
                    path TEXT,
                    album_id INTEGER,
                    title TEXT,
                    artist TEXT,
                    album TEXT,
                    track INTEGER,
                    disc INTEGER,
                    length REAL,
                    genre TEXT,
                    lyrics TEXT,
                    format TEXT,
                    bitrate INTEGER,
                    samplerate INTEGER,
                    bitdepth INTEGER,
                    channels INTEGER
                );
                INSERT INTO albums (id, album, year, artpath) VALUES (1, 'Brave Shine', 2015, 'D:/covers/cover.jpg');
                INSERT INTO items (id, path, album_id, title, artist, album, track, disc, length, genre, lyrics, format, bitrate, samplerate, bitdepth, channels)
                VALUES (1, 'D:/Music/brave-shine.flac', 1, 'Brave Shine', 'Aimer', 'Brave Shine', 1, 1, 212.0, 'J-Pop', 'fly high', 'FLAC', 900000, 48000, 24, 2);
                """
            )
            conn.commit()
            conn.close()

            migrate(str(beets_db), str(melo_db))

            out = sqlite3.connect(melo_db)
            song = out.execute("SELECT title, lyrics, format FROM songs").fetchone()
            album = out.execute("SELECT title, year FROM albums").fetchone()
            self.assertEqual(song, ("Brave Shine", "fly high", "FLAC"))
            self.assertEqual(album, ("Brave Shine", 2015))
            out.close()


if __name__ == "__main__":
    unittest.main()
