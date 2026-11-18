import { useParams } from '@tanstack/react-router';

import { RecordEditorPage } from './RecordEditorPage';

export function EditRecordPage() {
  const { recordId } = useParams({ strict: false });
  return <RecordEditorPage recordId={recordId} />;
}
