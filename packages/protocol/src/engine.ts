export const ENGINE_PROTOCOL_VERSION = 1 as const;
export const ENGINE_HANDSHAKE_PREFIX = 'SONASUB_HANDSHAKE ' as const;

export interface EngineInfo {
  engineVersion: string;
  nodeVersion: string;
  pid: number;
  startedAt: string;
}

export type EngineMethod = 'engine.info' | 'engine.shutdown';

export interface EngineRequest {
  protocolVersion: typeof ENGINE_PROTOCOL_VERSION;
  type: 'request';
  id: string;
  method: EngineMethod;
}

export interface EngineAuth {
  protocolVersion: typeof ENGINE_PROTOCOL_VERSION;
  type: 'auth';
  token: string;
}

export interface EngineHandshake {
  protocolVersion: typeof ENGINE_PROTOCOL_VERSION;
  port: number;
  pid: number;
}

export interface EngineError {
  code: string;
  message: string;
}

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

export class ProtocolError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = 'ProtocolError';
    this.code = code;
  }
}

function parseObject(raw: string): Record<string, unknown> {
  let value: unknown;
  try {
    value = JSON.parse(raw);
  } catch {
    throw new ProtocolError('invalid_json', '引擎消息不是有效 JSON。');
  }
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new ProtocolError('invalid_message', '引擎消息必须是对象。');
  }
  if (!('protocolVersion' in value) || value.protocolVersion !== ENGINE_PROTOCOL_VERSION) {
    throw new ProtocolError('unsupported_version', '引擎协议版本不受支持。');
  }
  return value;
}

function requireId(value: unknown): string {
  if (typeof value !== 'string' || value.length === 0 || value.length > 128) {
    throw new ProtocolError('invalid_request', '引擎消息缺少有效请求 ID。');
  }
  return value;
}

function requirePort(value: unknown): number {
  if (typeof value !== 'number' || !Number.isInteger(value) || value < 1 || value > 65535) {
    throw new ProtocolError('invalid_handshake', '引擎握手端口无效。');
  }
  return value;
}

function requirePid(value: unknown): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value <= 0) {
    throw new ProtocolError('invalid_message', '引擎进程 ID 无效。');
  }
  return value;
}

export function parseEngineRequest(raw: string): EngineRequest {
  const value = parseObject(raw);
  if (
    !('type' in value) ||
    value.type !== 'request' ||
    !('method' in value) ||
    (value.method !== 'engine.info' && value.method !== 'engine.shutdown')
  ) {
    throw new ProtocolError('invalid_request', '不支持的引擎控制请求。');
  }
  return {
    protocolVersion: ENGINE_PROTOCOL_VERSION,
    type: 'request',
    id: requireId('id' in value ? value.id : undefined),
    method: value.method,
  };
}

export function parseEngineAuth(raw: string): EngineAuth {
  const value = parseObject(raw);
  if (
    !('type' in value) ||
    value.type !== 'auth' ||
    !('token' in value) ||
    typeof value.token !== 'string' ||
    value.token.length === 0
  ) {
    throw new ProtocolError('invalid_auth', '引擎认证消息无效。');
  }
  return { protocolVersion: ENGINE_PROTOCOL_VERSION, type: 'auth', token: value.token };
}

export function parseEngineHandshake(raw: string): EngineHandshake {
  if (!raw.startsWith(ENGINE_HANDSHAKE_PREFIX)) {
    throw new ProtocolError('invalid_handshake', '这不是引擎启动握手。');
  }
  const value = parseObject(raw.slice(ENGINE_HANDSHAKE_PREFIX.length));
  return {
    protocolVersion: ENGINE_PROTOCOL_VERSION,
    port: requirePort('port' in value ? value.port : undefined),
    pid: requirePid('pid' in value ? value.pid : undefined),
  };
}

export function parseEngineResponse(raw: string, id: string, method: EngineMethod): EngineResponse {
  const value = parseObject(raw);
  if (!('type' in value) || value.type !== 'response' || !('id' in value) || value.id !== id) {
    throw new ProtocolError('request_mismatch', '引擎响应与请求关联信息不一致。');
  }
  if (!('ok' in value)) {
    throw new ProtocolError('invalid_response', '引擎响应缺少结果标记。');
  }
  if (value.ok === false) {
    if (
      typeof value.error !== 'object' ||
      value.error === null ||
      !('code' in value.error) ||
      !('message' in value.error) ||
      typeof value.error.code !== 'string' ||
      typeof value.error.message !== 'string'
    ) {
      throw new ProtocolError('invalid_response', '引擎错误响应无效。');
    }
    return {
      protocolVersion: ENGINE_PROTOCOL_VERSION,
      type: 'response',
      id,
      ok: false,
      error: { code: value.error.code, message: value.error.message },
    };
  }
  if (value.ok !== true) {
    throw new ProtocolError('invalid_response', '引擎响应缺少结果标记。');
  }
  if (method === 'engine.shutdown') {
    if (!('result' in value) || value.result !== null) {
      throw new ProtocolError('invalid_response', '引擎停止响应不应携带运行信息。');
    }
    return {
      protocolVersion: ENGINE_PROTOCOL_VERSION,
      type: 'response',
      id,
      ok: true,
      result: null,
    };
  }
  if (
    !('result' in value) ||
    typeof value.result !== 'object' ||
    value.result === null ||
    Array.isArray(value.result)
  ) {
    throw new ProtocolError('invalid_response', '引擎运行信息无效。');
  }
  const result = value.result;
  if (
    !('engineVersion' in result) ||
    typeof result.engineVersion !== 'string' ||
    result.engineVersion.length === 0 ||
    !('nodeVersion' in result) ||
    typeof result.nodeVersion !== 'string' ||
    result.nodeVersion.length === 0 ||
    !('startedAt' in result) ||
    typeof result.startedAt !== 'string' ||
    Number.isNaN(Date.parse(result.startedAt))
  ) {
    throw new ProtocolError('invalid_response', '引擎运行信息字段无效。');
  }
  return {
    protocolVersion: ENGINE_PROTOCOL_VERSION,
    type: 'response',
    id,
    ok: true,
    result: {
      engineVersion: result.engineVersion,
      nodeVersion: result.nodeVersion,
      pid: requirePid('pid' in result ? result.pid : undefined),
      startedAt: result.startedAt,
    },
  };
}
