"""usage: r120-edge-variant.py <library dir> <new dir>

Copies library.db and track-labels.json of a fixture library into <new dir>, links its anlz folder, and rewrites a
share of the tracks with the data that breaks a naive wire format: empty artist, album, genre, key and title, quotes,
backslashes and non-ASCII in titles and artists (a leading BOM, astral characters), Windows and bare paths, tempo 0,
tempos that round at the half step, analyses without a key or a row preview, stages 3, and a fifth of the covers gone.
"""
import os
import shutil
import sqlite3
import sys

src, dst = sys.argv[1], sys.argv[2]
os.makedirs(dst)
shutil.copy(os.path.join(src, "library.db"), os.path.join(dst, "library.db"))
shutil.copy(os.path.join(src, "track-labels.json"), os.path.join(dst, "track-labels.json"))
os.symlink(os.path.realpath(os.path.join(src, "anlz")), os.path.join(dst, "anlz"))
db = sqlite3.connect(os.path.join(dst, "library.db"))
statements = [
    "UPDATE tracks SET artist = '' WHERE id = (SELECT MIN(id) FROM tracks) OR id % 7 = 0",
    "UPDATE tracks SET album = '' WHERE id = (SELECT MIN(id) FROM tracks) OR id % 11 = 0",
    "UPDATE tracks SET genre = '' WHERE id = (SELECT MIN(id) FROM tracks) OR id % 13 = 0",
    "UPDATE tracks SET key = '' WHERE id % 17 = 0",
    "UPDATE tracks SET title = '' WHERE id = (SELECT MIN(id) FROM tracks) OR id % 19 = 0",
    "UPDATE tracks SET title = 'He said \"hi\" \\ ' || id WHERE id % 23 = 0",
    "UPDATE tracks SET title = 'Ünï — 日本語 😀 ' || id, album = 'アルバム ' || id % 50 WHERE id % 29 = 0",
    "UPDATE tracks SET artist = 'Beyoncé' WHERE id % 31 = 0",
    "UPDATE tracks SET title = char(65279) || 'BOM ' || id WHERE id % 211 = 0",
    "UPDATE tracks SET genre = '/\\\"' WHERE id % 53 = 0",
    "UPDATE tracks SET source_path = 'C:\\Music\\Ünï\\' || id || '.mp3' WHERE id % 37 = 0",
    "UPDATE tracks SET source_path = 'bare-' || id || '.mp3' WHERE id % 101 = 0",
    "UPDATE tracks SET source_path = '/m/日本/曲 ' || id || '.mp3' WHERE id % 61 = 0",
    "UPDATE tracks SET tempo = 0 WHERE id % 41 = 0",
    "UPDATE analyses SET bpm = 0 WHERE track_id % 41 = 0",
    "UPDATE analyses SET bpm = 127.995 WHERE track_id % 43 = 0",
    "UPDATE analyses SET bpm = 128.005 WHERE track_id % 47 = 0",
    "UPDATE analyses SET key = '' WHERE track_id % 17 = 0",
    "UPDATE analyses SET stages = 3 WHERE track_id % 59 = 0",
    "UPDATE analyses SET row_preview = NULL WHERE track_id % 33 = 0",
    "DELETE FROM artwork WHERE track_id % 5 = 0",
]
for statement in statements:
    db.execute(statement)
db.commit()
db.execute("PRAGMA wal_checkpoint(TRUNCATE)")
print(db.execute("SELECT COUNT(*) FROM tracks").fetchone()[0], "tracks")
