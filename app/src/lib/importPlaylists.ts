export type PlannedPlaylist =
  | { kind: 'playlist'; name: string; files: string[] }
  | { kind: 'folder'; name: string; children: PlannedPlaylist[] };

export interface DroppedAudio {
  path: string;
  folder: boolean;
  files: string[];
}

interface FolderNode {
  name: string;
  files: string[];
  children: Map<string, FolderNode>;
}

function normalizePath(path: string): string {
  if (path.indexOf('\\') < 0 && !path.endsWith('/')) return path;
  return path.replace(/\\/g, '/').replace(/\/+$/, '');
}

function lastSegment(path: string): string {
  return normalizePath(path).split('/').pop() ?? '';
}

const byName = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });

function planFor(node: FolderNode): PlannedPlaylist | null {
  const children = [...node.children.values()]
    .sort((a, b) => byName.compare(a.name, b.name))
    .map(planFor)
    .filter((plan): plan is PlannedPlaylist => plan !== null);
  if (children.length === 0) return node.files.length > 0 ? { kind: 'playlist', name: node.name, files: node.files } : null;
  const own: PlannedPlaylist[] = node.files.length > 0 ? [{ kind: 'playlist', name: node.name, files: node.files }] : [];
  return { kind: 'folder', name: node.name, children: [...own, ...children] };
}

/**
 * Mirrors a folder of audio files as playlists. A folder without subfolders is one playlist; a folder
 * with subfolders becomes a playlist folder holding a playlist per subfolder, plus one named after
 * itself for the files that sit directly in it. Folders without audio are left out.
 */
export function folderPlan(root: string, files: readonly string[]): PlannedPlaylist | null {
  const base = normalizePath(root);
  const top: FolderNode = { name: lastSegment(base) || base, files: [], children: new Map() };
  for (const file of files) {
    const path = normalizePath(file);
    if (!path.startsWith(`${base}/`)) continue;
    const segments = path.slice(base.length + 1).split('/');
    let node = top;
    for (const dir of segments.slice(0, -1)) {
      let child = node.children.get(dir);
      if (!child) {
        child = { name: dir, files: [], children: new Map() };
        node.children.set(dir, child);
      }
      node = child;
    }
    node.files.push(file);
  }
  return planFor(top);
}

/** Playlists for a drop: one tree per dropped folder, and the loose files together in one playlist. */
export function dropPlan(dropped: readonly DroppedAudio[], looseName: string): PlannedPlaylist[] {
  const plans = dropped
    .filter((d) => d.folder)
    .map((d) => folderPlan(d.path, d.files))
    .filter((plan): plan is PlannedPlaylist => plan !== null);
  const loose = dropped.filter((d) => !d.folder).flatMap((d) => d.files);
  if (loose.length > 0) plans.push({ kind: 'playlist', name: looseName, files: loose });
  return plans;
}

export function plannedFileCount(plans: readonly PlannedPlaylist[]): number {
  return plans.reduce((sum, plan) => sum + (plan.kind === 'playlist' ? plan.files.length : plannedFileCount(plan.children)), 0);
}

/** The placeholder names shown for files that are still being imported: file names without extension, sorted. */
export function pendingNames(files: readonly string[]): string[] {
  return files.map((file) => lastSegment(file).replace(/\.[^.]+$/, '')).sort((a, b) => byName.compare(a, b));
}

/**
 * The library ids of a playlist's files, in library order. `matches` maps files that were skipped as
 * duplicates to the library track they duplicate. `wanted` is every file the returned function will be asked about;
 * only those paths are looked up, so the library is read once without keeping a path for every row.
 */
export function trackIdsForFiles(
  tracks: ReadonlyArray<{ id: number; filePath?: string }>,
  matches: ReadonlyArray<{ path: string; id: number }>,
  wanted: readonly string[],
): (files: readonly string[]) => number[] {
  const wantedPaths = new Set(wanted.map(normalizePath));
  const matched = new Set(matches.map((match) => match.id));
  const order = new Map<number, number>();
  const byPath = new Map<string, number>();
  for (let index = 0; index < tracks.length; index++) {
    const track = tracks[index];
    const path = track.filePath ? normalizePath(track.filePath) : null;
    const found = path !== null && wantedPaths.has(path);
    if (found) byPath.set(path, track.id);
    if (found || matched.has(track.id)) order.set(track.id, index);
  }
  for (const match of matches) byPath.set(normalizePath(match.path), match.id);
  return (files) => {
    const ids = new Set<number>();
    for (const file of files) {
      const id = byPath.get(normalizePath(file));
      if (id !== undefined) ids.add(id);
    }
    return [...ids].sort((a, b) => (order.get(a) ?? Infinity) - (order.get(b) ?? Infinity));
  };
}
