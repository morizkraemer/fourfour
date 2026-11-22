import type { ArchivedMerge, DuplicateGroup, DuplicateMember, MergeReport, MergeRequest } from '../services/tauri.svelte.ts';

export type KeeperChoices = Record<number, number>;

function count(n: number, word: string, plural = `${word}s`): string {
  return `${n} ${n === 1 ? word : plural}`;
}

export function confidenceLabel(group: DuplicateGroup): string {
  return group.confidence === 'exact_audio' ? 'Identical audio' : 'Same title and length, different audio';
}

export function groupsTitle(groups: number): string {
  return groups === 1
    ? '1 song is in your library more than once. Pick the copy to keep.'
    : `${groups} songs are in your library more than once. Tick the songs to merge and pick the copy to keep.`;
}

export function groupNote(group: DuplicateGroup, choices: KeeperChoices): string {
  return `${confidenceLabel(group)} · ${keeperOf(group, choices) === group.keeper_id ? `Suggested: ${group.reason}` : 'Your pick'}`;
}

function quality(copy: { format: string; lossless: boolean; bitrate: number }): string {
  const format = copy.format ? copy.format.toUpperCase() : 'File';
  return copy.lossless || !(copy.bitrate > 0) ? format : `${format} ${copy.bitrate} kbps`;
}

function timeOf(secs: number): string {
  const whole = Math.round(secs);
  return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, '0')}`;
}

export function copyFacts(member: DuplicateMember, group: DuplicateGroup): string {
  const facts = [quality(member)];
  if (group.members.some((m) => Math.round(m.duration_secs) !== Math.round(member.duration_secs))) facts.push(timeOf(member.duration_secs));
  if (member.playlists === 0 && member.cues === 0) facts.push('no playlists or cues');
  else {
    if (member.playlists > 0) facts.push(`in ${count(member.playlists, 'playlist')}`);
    if (member.cues > 0) facts.push(count(member.cues, 'cue'));
  }
  return facts.join(' · ');
}

function segments(path: string): string[] {
  return path.split(/[\\/]/).filter(Boolean);
}

function withoutHome(folder: string[]): string[] {
  if (folder[0] === 'Volumes') return folder.slice(1);
  return folder[0] === 'Users' || folder[0] === 'home' ? folder.slice(2) : folder;
}

export function copyPlaces(paths: string[]): string[] {
  const split = paths.map(segments);
  const folders = split.map((s) => s.slice(0, -1));
  if (folders.every((f) => f.join('/') === folders[0].join('/'))) return split.map((s) => s[s.length - 1] ?? '');
  let shared = 0;
  while (folders.every((f) => shared < f.length && f[shared] === folders[0][shared])) shared += 1;
  if (shared === 0) return folders.map((f) => withoutHome(f).join('/'));
  const from = folders.some((f) => f.length === shared) ? shared - 1 : shared;
  return folders.map((f) => f.slice(from).join('/'));
}

export function groupHeading(group: DuplicateGroup): string {
  const first = group.members.find((m) => m.id === group.keeper_id) ?? group.members[0];
  if (!first) return '';
  return first.artist ? `${first.artist} - ${first.title}` : first.title;
}

export function keeperOf(group: DuplicateGroup, choices: KeeperChoices): number {
  const chosen = choices[group.key];
  return group.members.some((m) => m.id === chosen) ? chosen : group.keeper_id;
}

export function requestFor(group: DuplicateGroup, choices: KeeperChoices): MergeRequest {
  const keeper = keeperOf(group, choices);
  return {
    keeper_id: keeper,
    remove_ids: group.members.filter((m) => m.id !== keeper).map((m) => m.id),
    require_identical: group.confidence === 'exact_audio',
  };
}

export function defaultPicks(groups: DuplicateGroup[]): Set<number> {
  return new Set(groups.filter((g) => g.confidence === 'exact_audio').map((g) => g.key));
}

export function pickedRequests(groups: DuplicateGroup[], choices: KeeperChoices, picked: ReadonlySet<number>): MergeRequest[] {
  return groups.filter((g) => picked.has(g.key)).map((g) => requestFor(g, choices));
}

export function pickedCount(groups: DuplicateGroup[], picked: ReadonlySet<number>): number {
  return groups.filter((g) => picked.has(g.key)).length;
}

export function mergeLabel(songs: number): string {
  if (songs === 0) return 'Merge';
  return songs === 1 ? 'Merge 1 song' : `Merge ${songs} songs`;
}

export function groupsAfterMerge(groups: DuplicateGroup[], report: MergeReport): DuplicateGroup[] {
  const done = new Set(report.merged.map((m) => m.keeper_id));
  const removed = new Set(report.merged.flatMap((m) => m.removed_ids));
  return groups.filter((g) => !g.members.some((m) => done.has(m.id) || removed.has(m.id)));
}

export function mergeSummary(report: MergeReport): string {
  const songs = report.merged.length;
  const entries = report.merged.reduce((sum, m) => sum + m.removed_ids.length, 0);
  const parts: string[] = [];
  if (songs > 0) parts.push(`Merged ${count(entries, 'copy', 'copies')} into ${count(songs, 'keeper')}. No file was deleted.`);
  if (report.failed.length > 0) parts.push(`${count(report.failed.length, 'merge')} not done: ${report.failed[0].message}`);
  return parts.join(' ');
}

export function archiveHeading(merge: ArchivedMerge): string {
  return merge.keeper_label ?? 'Merged song';
}

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

export function archiveDate(merge: ArchivedMerge): string {
  const [year, month, day] = merge.merged_at.slice(0, 10).split('-').map(Number);
  return MONTHS[month - 1] ? `${day} ${MONTHS[month - 1]} ${year}` : merge.merged_at.slice(0, 10);
}

export function archiveNote(merge: ArchivedMerge): string {
  return `${merge.tracks.length === 1 ? archivedCopy(merge.tracks[0]) : count(merge.tracks.length, 'copy', 'copies')} · merged ${archiveDate(merge)}`;
}

function archivedCopy(track: ArchivedMerge['tracks'][number]): string {
  const folder = segments(track.path).slice(-2, -1)[0];
  return folder ? `${quality(track)} from ${folder}` : quality(track);
}
