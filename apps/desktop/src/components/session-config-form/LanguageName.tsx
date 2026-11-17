import { useTranslation } from '@sonasub/i18n';

type LanguageNameProps = {
  code: string;
};

export function LanguageName({ code }: LanguageNameProps) {
  const { t } = useTranslation();
  if (code === 'auto') return t(($) => $.languages.auto);
  if (code === 'zh' || code === 'zh-Hans') return t(($) => $.languages.chinese);
  if (code === 'en') return t(($) => $.languages.english);
  if (code === 'ja') return t(($) => $.languages.japanese);
  if (code === 'ko') return t(($) => $.languages.korean);
  if (code === 'yue') return t(($) => $.languages.cantonese);
  return code;
}
