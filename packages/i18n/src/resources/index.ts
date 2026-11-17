import type { Locale } from '../i18n';
import { enUs } from './en-US';
import type { TranslationResource } from './en-US';
import type { TranslationShape } from './translation.type';
import { zhCn } from './zh-CN';

export { enUs } from './en-US';
export { zhCn } from './zh-CN';
export type { TranslationResource } from './en-US';

export const resources = {
  'en-US': { translation: enUs },
  'zh-CN': { translation: zhCn },
} as const satisfies Record<Locale, { translation: TranslationShape<TranslationResource> }>;
