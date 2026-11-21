import {
  applyDuplicateMerges,
  listDuplicateArchive,
  listDuplicateGroups,
  restoreDuplicateMerge,
  type ArchivedMerge,
  type DuplicateGroup,
  type MergeRequest,
} from '../services/tauri.svelte.ts';
import { exactRequests, groupsAfterMerge, mergeSummary, requestFor, type KeeperChoices } from '../lib/duplicateCleanup.ts';
import { failureText, rawText } from '../lib/userErrors.ts';
import { loadState } from './library.svelte.ts';

export const duplicates = $state({
  open: false,
  tab: 'groups' as 'groups' | 'archive',
  loading: false,
  busy: false,
  groups: [] as DuplicateGroup[],
  choices: {} as KeeperChoices,
  skipped: new Set<number>(),
  archive: [] as ArchivedMerge[],
  message: '',
  error: '',
});

async function refreshArchive() {
  duplicates.archive = await listDuplicateArchive();
}

export async function openDuplicates() {
  duplicates.open = true;
  duplicates.tab = 'groups';
  duplicates.message = '';
  duplicates.error = '';
  duplicates.loading = true;
  duplicates.choices = {};
  duplicates.skipped = new Set();
  try {
    const [groups] = await Promise.all([listDuplicateGroups(), refreshArchive()]);
    duplicates.groups = groups;
  } catch (err) {
    duplicates.error = failureText("Couldn't look for duplicates.", err);
  } finally {
    duplicates.loading = false;
  }
}

export function closeDuplicates() {
  duplicates.open = false;
}

export function showTab(tab: 'groups' | 'archive') {
  duplicates.tab = tab;
}

export function chooseKeeper(group: DuplicateGroup, trackId: number) {
  duplicates.choices = { ...duplicates.choices, [group.key]: trackId };
}

export function toggleSkip(group: DuplicateGroup) {
  const next = new Set(duplicates.skipped);
  if (next.has(group.key)) next.delete(group.key);
  else next.add(group.key);
  duplicates.skipped = next;
}

async function merge(requests: MergeRequest[]) {
  if (requests.length === 0 || duplicates.busy) return;
  duplicates.busy = true;
  duplicates.error = '';
  try {
    const report = await applyDuplicateMerges(requests);
    duplicates.groups = groupsAfterMerge(duplicates.groups, report);
    duplicates.message = mergeSummary(report);
    await Promise.all([refreshArchive(), loadState()]);
  } catch (err) {
    duplicates.error = failureText("Couldn't merge the copies.", err);
  } finally {
    duplicates.busy = false;
  }
}

export function mergeGroup(group: DuplicateGroup) {
  return merge([requestFor(group, duplicates.choices)]);
}

export function mergeAllExact() {
  return merge(exactRequests(duplicates.groups, duplicates.choices, duplicates.skipped));
}

export async function restoreMerge(mergeId: number) {
  if (duplicates.busy) return;
  duplicates.busy = true;
  duplicates.error = '';
  try {
    const outcome = await restoreDuplicateMerge(mergeId);
    duplicates.message = `Restored ${outcome.restored_ids.length === 1 ? '1 track' : `${outcome.restored_ids.length} tracks`} with playlists and cues.`;
    await loadState();
    const [groups] = await Promise.all([listDuplicateGroups(), refreshArchive()]);
    duplicates.groups = groups;
  } catch (err) {
    duplicates.error = `Couldn't restore the copies. ${rawText(err)}`;
  } finally {
    duplicates.busy = false;
  }
}
