import type { Locale } from '../i18n.type';
import { enUs } from './en-US';
import type { TranslationResource } from './en-US';
import type { TranslationShape } from './translation.type';
import { zhCn } from './zh-CN';

export const resources = {
  'en-US': { translation: enUs },
  'zh-CN': { translation: zhCn },
} as const satisfies Record<Locale, { translation: TranslationShape<TranslationResource> }>;
