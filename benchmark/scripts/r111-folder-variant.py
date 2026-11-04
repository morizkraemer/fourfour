"""usage: r111-folder-variant.py <library dir> <new dir> <folders>

Copies library.db and track-labels.json of a fixture library into <new dir>, links its anlz folder, and rewrites
tracks.source_path so that the tracks (in id order) sit in <folders> album folders (five per artist folder), like a
real library, where the fixture has about 0.8 folders per track.
"""
import os
import shutil
import sqlite3
import sys

src, dst, folders = sys.argv[1], sys.argv[2], int(sys.argv[3])
os.makedirs(dst)
shutil.copy(os.path.join(src, "library.db"), os.path.join(dst, "library.db"))
shutil.copy(os.path.join(src, "track-labels.json"), os.path.join(dst, "track-labels.json"))
os.symlink(os.path.realpath(os.path.join(src, "anlz")), os.path.join(dst, "anlz"))
db = sqlite3.connect(os.path.join(dst, "library.db"))
rows = db.execute("SELECT id, source_path FROM tracks ORDER BY id").fetchall()
per_folder = -(-len(rows) // folders)
album = 0
for position, (track_id, source_path) in enumerate(rows):
    album = position // per_folder
    name = source_path.rsplit("/", 1)[-1]
    db.execute(
        "UPDATE tracks SET source_path = ? WHERE id = ?",
        (f"/Users/dj/Music/Artist {album // 5}/Album {album}/{name}", track_id),
    )
db.commit()
db.execute("PRAGMA wal_checkpoint(TRUNCATE)")
print(len(rows), "tracks in", album + 1, "folders")
