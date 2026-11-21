import type { ArchivedMerge, DuplicateGroup, DuplicateMember, MergeReport, MergeRequest } from '../services/tauri.svelte.ts';
import { trackTime } from './importDuplicates.ts';

export type KeeperChoices = Record<number, number>;

function count(n: number, word: string, plural = `${word}s`): string {
  return `${n} ${n === 1 ? word : plural}`;
}

export function confidenceLabel(group: DuplicateGroup): string {
  return group.confidence === 'exact_audio' ? 'Exact copy' : 'Same artist, title and length';
}

export function groupsTitle(groups: number): string {
  return groups === 1 ? '1 song has copies' : `${groups} songs have copies`;
}

export function groupHeading(group: DuplicateGroup): string {
  const first = group.members.find((m) => m.id === group.keeper_id) ?? group.members[0];
  if (!first) return '';
  return first.artist ? `${first.artist} - ${first.title}` : first.title;
}

export function memberDetail(member: DuplicateMember): string {
  const parts = [member.format || 'file'];
  if (member.bitrate > 0) parts.push(`${member.bitrate} kbps`);
  parts.push(trackTime(member.duration_secs));
  parts.push(count(member.playlists, 'playlist'));
  if (member.cues > 0) parts.push(count(member.cues, 'cue'));
  if (!member.file_present) parts.push('file missing');
  return parts.join(' · ');
}

export function keeperOf(group: DuplicateGroup, choices: KeeperChoices): number {
  const chosen = choices[group.key];
  return group.members.some((m) => m.id === chosen) ? chosen : group.keeper_id;
}

export function reasonOf(group: DuplicateGroup, choices: KeeperChoices): string {
  return keeperOf(group, choices) === group.keeper_id ? group.reason : 'your pick';
}

export function requestFor(group: DuplicateGroup, choices: KeeperChoices): MergeRequest {
  const keeper = keeperOf(group, choices);
  return {
    keeper_id: keeper,
    remove_ids: group.members.filter((m) => m.id !== keeper).map((m) => m.id),
    require_identical: group.confidence === 'exact_audio',
  };
}

export function exactRequests(groups: DuplicateGroup[], choices: KeeperChoices, skipped: ReadonlySet<number>): MergeRequest[] {
  return groups.filter((g) => g.confidence === 'exact_audio' && !skipped.has(g.key)).map((g) => requestFor(g, choices));
}

export function exactCount(groups: DuplicateGroup[], skipped: ReadonlySet<number>): number {
  return groups.filter((g) => g.confidence === 'exact_audio' && !skipped.has(g.key)).length;
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

export function archiveCopies(merge: ArchivedMerge): string {
  return count(merge.tracks.length, 'copy', 'copies');
}

export function archiveDate(merge: ArchivedMerge): string {
  return merge.merged_at.slice(0, 10);
}
