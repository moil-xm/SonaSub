import type {
  EngineAuth,
  EngineHandshake,
  EngineMethod,
  EngineRequest,
  EngineResponse,
} from './engine.type';

export const ENGINE_PROTOCOL_VERSION = 1 as const;
export const ENGINE_HANDSHAKE_PREFIX = 'SONASUB_HANDSHAKE ' as const;

export type {
  EngineAuth,
  EngineError,
  EngineHandshake,
  EngineInfo,
  EngineMethod,
  EngineRequest,
  EngineResponse,
} from './engine.type';

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
    throw new ProtocolError('invalid_json', 'The engine message is not valid JSON.');
  }
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new ProtocolError('invalid_message', 'The engine message must be an object.');
  }
  if (!('protocolVersion' in value) || value.protocolVersion !== ENGINE_PROTOCOL_VERSION) {
    throw new ProtocolError('unsupported_version', 'The engine protocol version is unsupported.');
  }
  return value;
}

function requireId(value: unknown): string {
  if (typeof value !== 'string' || value.length === 0 || value.length > 128) {
    throw new ProtocolError('invalid_request', 'The engine message is missing a valid request id.');
  }
  return value;
}

function requirePort(value: unknown): number {
  if (typeof value !== 'number' || !Number.isInteger(value) || value < 1 || value > 65535) {
    throw new ProtocolError('invalid_handshake', 'The engine handshake port is invalid.');
  }
  return value;
}

function requirePid(value: unknown): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value <= 0) {
    throw new ProtocolError('invalid_message', 'The engine process id is invalid.');
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
    throw new ProtocolError('invalid_request', 'Unsupported engine control request.');
  }
  return {
    protocolVersion: ENGINE_PROTOCOL_VERSION,
    type: 'request',
    id: requireId(value.id),
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
    throw new ProtocolError('invalid_auth', 'The engine authentication message is invalid.');
  }
  return { protocolVersion: ENGINE_PROTOCOL_VERSION, type: 'auth', token: value.token };
}

export function parseEngineHandshake(raw: string): EngineHandshake {
  if (!raw.startsWith(ENGINE_HANDSHAKE_PREFIX)) {
    throw new ProtocolError('invalid_handshake', 'This is not an engine startup handshake.');
  }
  const value = parseObject(raw.slice(ENGINE_HANDSHAKE_PREFIX.length));
  return {
    protocolVersion: ENGINE_PROTOCOL_VERSION,
    port: requirePort(value.port),
    pid: requirePid(value.pid),
  };
}

export function parseEngineResponse(raw: string, id: string, method: EngineMethod): EngineResponse {
  const value = parseObject(raw);
  if (!('type' in value) || value.type !== 'response' || !('id' in value) || value.id !== id) {
    throw new ProtocolError(
      'request_mismatch',
      'The engine response does not match the request id.',
    );
  }
  if (!('ok' in value)) {
    throw new ProtocolError('invalid_response', 'The engine response is missing the result flag.');
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
      throw new ProtocolError('invalid_response', 'The engine error response is invalid.');
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
    throw new ProtocolError('invalid_response', 'The engine response is missing the result flag.');
  }
  if (method === 'engine.shutdown') {
    if (!('result' in value) || value.result !== null) {
      throw new ProtocolError(
        'invalid_response',
        'An engine shutdown response must not carry runtime information.',
      );
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
    throw new ProtocolError('invalid_response', 'The engine runtime information is invalid.');
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
    Number.isNaN(Date.parse(result.startedAt)) ||
    !('pid' in result)
  ) {
    throw new ProtocolError(
      'invalid_response',
      'The engine runtime information fields are invalid.',
    );
  }
  return {
    protocolVersion: ENGINE_PROTOCOL_VERSION,
    type: 'response',
    id,
    ok: true,
    result: {
      engineVersion: result.engineVersion,
      nodeVersion: result.nodeVersion,
      pid: requirePid(result.pid),
      startedAt: result.startedAt,
    },
  };
}
