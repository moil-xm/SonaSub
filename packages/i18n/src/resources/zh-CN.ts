import type { TranslationResource } from './en-US';
import type { TranslationShape } from './translation.type';

export const zhCn = {
  navigation: {
    overview: '概览',
    about: '关于',
  },
  theme: {
    switchToDark: '切换到深色模式',
    switchToLight: '切换到浅色模式',
  },
  language: {
    label: '语言',
    followSystem: '跟随系统',
    english: 'English',
    chinese: '简体中文',
  },
  overview: {
    title: '运行概览',
    description: '管理本地引擎的启动、连接与关闭。',
  },
  engine: {
    title: '本地引擎',
    description:
      '桌面应用负责管理引擎，并通过回环连接展示真实运行信息。音频、语音识别与翻译尚未接入。',
    states: {
      stopped: '已停止',
      starting: '启动中',
      running: '运行中',
      stopping: '停止中',
      failed: '启动或运行失败',
    },
    status: {
      checking: '正在检查',
      queryFailed: '状态查询失败',
    },
    actions: {
      start: '启动',
      stop: '停止',
      restart: '重启',
    },
    errors: {
      failed: '引擎运行失败',
      operationFailed: '引擎操作失败',
      statusQueryFailed: '引擎状态查询失败',
      unknown: '未知的引擎错误',
    },
    details: {
      version: '引擎版本',
      startedAt: '启动时间',
    },
    diagnostics: {
      title: '开发诊断',
    },
  },
  extension: {
    description: '扩展入口已就绪，字幕功能尚未接入。',
    roadmap: '多标签页处理、画面采集与识别流程将在后续任务中实现。',
  },
  about: {
    description: 'SonaSub 是一个开源实时字幕项目。此页面渲染于 {{time}}。',
    moduleCount_one: '{{count}} 个模块',
    moduleCount_other: '{{count}} 个模块',
    modules: {
      desktop: '桌面前端与 src-tauri 原生后端',
      engine: '提供真实运行信息的回环 WebSocket 引擎',
      extension: 'Chrome 扩展',
      ui: '共享组件、主题、图标与样式',
      protocol: '桌面后端与引擎的通信契约',
      i18n: '共享界面翻译与类型安全的 i18next 配置',
    },
  },
} as const satisfies TranslationShape<TranslationResource>;
