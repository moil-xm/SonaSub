import { create } from 'zustand';

import type { RecordEditorDraft } from './record-editor-draft.type';

type RecordEditorDraftState = {
  draft: RecordEditorDraft | null;
  clearDraft: (key: string) => void;
  saveDraft: (draft: RecordEditorDraft) => void;
};

const useRecordEditorDraftStore = create<RecordEditorDraftState>((set) => ({
  draft: null,
  clearDraft: (key) =>
    set((state) => {
      if (state.draft?.key !== key) return state;
      return { draft: null };
    }),
  saveDraft: (draft) => set({ draft }),
}));

export function readRecordEditorDraft(key: string): RecordEditorDraft | null {
  const draft = useRecordEditorDraftStore.getState().draft;
  if (draft?.key !== key) return null;
  return draft;
}

export function saveRecordEditorDraft(
  key: string,
  name: string,
  config: RecordEditorDraft['config'],
) {
  useRecordEditorDraftStore.getState().saveDraft({
    key,
    name,
    config,
    updatedAtMs: Date.now(),
  });
}

export function clearRecordEditorDraft(key: string) {
  useRecordEditorDraftStore.getState().clearDraft(key);
}
