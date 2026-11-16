import type { ENGINE_PROTOCOL_VERSION } from './engine';

export type EngineInfo = {
  engineVersion: string;
  nodeVersion: string;
  pid: number;
  startedAt: string;
};

export type EngineMethod = 'engine.info' | 'engine.shutdown';

export type EngineRequest = {
  protocolVersion: typeof ENGINE_PROTOCOL_VERSION;
  type: 'request';
  id: string;
  method: EngineMethod;
};

export type EngineAuth = {
  protocolVersion: typeof ENGINE_PROTOCOL_VERSION;
  type: 'auth';
  token: string;
};

export type EngineHandshake = {
  protocolVersion: typeof ENGINE_PROTOCOL_VERSION;
  port: number;
  pid: number;
};

export type EngineError = {
  code: string;
  message: string;
};

export type EngineResponse =
  | {
      protocolVersion: typeof ENGINE_PROTOCOL_VERSION;
      type: 'response';
      id: string;
      ok: true;
      result: EngineInfo | null;
    }
  | {
      protocolVersion: typeof ENGINE_PROTOCOL_VERSION;
      type: 'response';
      id: string;
      ok: false;
      error: EngineError;
    };
