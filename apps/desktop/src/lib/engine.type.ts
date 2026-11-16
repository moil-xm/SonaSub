import type { EngineInfo } from '@sonasub/protocol/engine';

export type EngineState =
  | { state: 'stopped' }
  | { state: 'starting'; pid: number | null }
  | { state: 'running'; info: EngineInfo; port: number }
  | { state: 'stopping'; info: EngineInfo; port: number }
  | { state: 'failed'; error: string };

export type EngineAction = 'start' | 'stop' | 'restart';
