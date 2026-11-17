import type { ReadinessIssue } from '@/lib/model-readiness';

import { ReadinessMessage } from './ReadinessMessage';

export type ReadinessIssuesProps = {
  issues: ReadinessIssue[];
};

export function ReadinessIssues({ issues }: ReadinessIssuesProps) {
  if (issues.length === 0) return null;
  return (
    <ul className="list-disc space-y-1 pl-4 text-sm">
      {issues.map((issue) => (
        <ReadinessMessage key={issue} issue={issue} />
      ))}
    </ul>
  );
}
