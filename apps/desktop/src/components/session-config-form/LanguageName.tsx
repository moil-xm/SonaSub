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
  if (code === 'ar') return t(($) => $.languages.arabic);
  if (code === 'de') return t(($) => $.languages.german);
  if (code === 'fr') return t(($) => $.languages.french);
  if (code === 'es') return t(($) => $.languages.spanish);
  if (code === 'pt') return t(($) => $.languages.portuguese);
  if (code === 'id') return t(($) => $.languages.indonesian);
  if (code === 'it') return t(($) => $.languages.italian);
  if (code === 'ru') return t(($) => $.languages.russian);
  if (code === 'th') return t(($) => $.languages.thai);
  if (code === 'vi') return t(($) => $.languages.vietnamese);
  if (code === 'tr') return t(($) => $.languages.turkish);
  if (code === 'hi') return t(($) => $.languages.hindi);
  if (code === 'ms') return t(($) => $.languages.malay);
  if (code === 'nl') return t(($) => $.languages.dutch);
  if (code === 'sv') return t(($) => $.languages.swedish);
  if (code === 'da') return t(($) => $.languages.danish);
  if (code === 'fi') return t(($) => $.languages.finnish);
  if (code === 'pl') return t(($) => $.languages.polish);
  if (code === 'cs') return t(($) => $.languages.czech);
  if (code === 'fil') return t(($) => $.languages.filipino);
  if (code === 'fa') return t(($) => $.languages.persian);
  if (code === 'el') return t(($) => $.languages.greek);
  if (code === 'hu') return t(($) => $.languages.hungarian);
  if (code === 'mk') return t(($) => $.languages.macedonian);
  if (code === 'ro') return t(($) => $.languages.romanian);
  return code;
}
