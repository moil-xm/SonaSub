import { useTranslation } from '@sonasub/i18n';

import type { ReadinessIssue } from '@/lib/model-readiness';

type ReadinessMessageProps = {
  issue: ReadinessIssue;
};

export function ReadinessMessage({ issue }: ReadinessMessageProps) {
  const { t } = useTranslation();

  let message: string = t(($) => $.workbench.readiness.unknownAsrModel);
  if (issue === 'unsupportedPlatform')
    message = t(($) => $.workbench.readiness.unsupportedPlatform);
  if (issue === 'asrModel') message = t(($) => $.workbench.readiness.asrModel);
  if (issue === 'asrMode') message = t(($) => $.workbench.readiness.asrMode);
  if (issue === 'asrLanguage') message = t(($) => $.workbench.readiness.asrLanguage);
  if (issue === 'vadModel') message = t(($) => $.workbench.readiness.vadModel);
  if (issue === 'translationEngine') message = t(($) => $.workbench.readiness.translationEngine);
  if (issue === 'translationTarget') message = t(($) => $.workbench.readiness.translationTarget);
  if (issue === 'translationConfig') message = t(($) => $.workbench.readiness.translationConfig);
  if (issue === 'translationModel') message = t(($) => $.workbench.readiness.translationModel);
  if (issue === 'speakerModel') message = t(($) => $.workbench.readiness.speakerModel);
  if (issue === 'diarizationModel') message = t(($) => $.workbench.readiness.diarizationModel);

  return <li>{message}</li>;
}
