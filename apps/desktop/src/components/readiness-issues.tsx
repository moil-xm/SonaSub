import { useTranslation } from '@sonasub/i18n';

import type { ReadinessIssue } from '@/lib/model-readiness';

type ReadinessMessageProps = {
  issue: ReadinessIssue;
};

function ReadinessMessage({ issue }: ReadinessMessageProps) {
  const { t } = useTranslation();

  let message: string = t(($) => $.workbench.readiness.unknownAsrModel);
  if (issue === 'unsupportedPlatform')
    message = t(($) => $.workbench.readiness.unsupportedPlatform);
  if (issue === 'asrModel') message = t(($) => $.workbench.readiness.asrModel);
  if (issue === 'asrMode') message = t(($) => $.workbench.readiness.asrMode);
  if (issue === 'vadModel') message = t(($) => $.workbench.readiness.vadModel);
  if (issue === 'translationModel') message = t(($) => $.workbench.readiness.translationModel);
  if (issue === 'speakerModel') message = t(($) => $.workbench.readiness.speakerModel);
  if (issue === 'diarizationModel') message = t(($) => $.workbench.readiness.diarizationModel);

  return <li>{message}</li>;
}

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
