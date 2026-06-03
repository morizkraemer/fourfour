export function getArtboardNotes(documentState, artboardId) {
  return (documentState.notes || []).filter(
    (note) => note.target?.type === 'artboard' && note.target.artboardId === artboardId
  );
}

export function getPrimaryArtboardNote(documentState, artboardId) {
  return getArtboardNotes(documentState, artboardId)[0] || null;
}

function createNoteId() {
  return `note-${Math.random().toString(16).slice(2)}${Date.now().toString(16)}`;
}

export function upsertArtboardPrimaryNote(draft, artboardId, text) {
  const trimmed = (text || '').trim();
  const notes = Array.isArray(draft.notes) ? draft.notes : [];
  const existingIndex = notes.findIndex(
    (note) => note.target?.type === 'artboard' && note.target.artboardId === artboardId
  );

  if (!trimmed) {
    if (existingIndex >= 0) notes.splice(existingIndex, 1);
    draft.notes = notes;
    return;
  }

  const now = new Date().toISOString();
  if (existingIndex >= 0) {
    notes[existingIndex] = {
      ...notes[existingIndex],
      text: trimmed,
      updatedAt: now,
    };
    draft.notes = notes;
    return;
  }

  notes.push({
    id: createNoteId(),
    target: { type: 'artboard', artboardId },
    text: trimmed,
    canvasOffset: { x: 8, y: -32 },
    createdAt: now,
    updatedAt: now,
  });
  draft.notes = notes;
}
