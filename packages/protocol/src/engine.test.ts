import { describe, expect, it } from 'vite-plus/test';

import {
  ENGINE_HANDSHAKE_PREFIX,
  ENGINE_PROTOCOL_VERSION,
  ProtocolError,
  parseEngineAuth,
  parseEngineHandshake,
  parseEngineRequest,
  parseEngineResponse,
} from './engine';

describe('engine protocol', () => {
  it('parses authenticated control requests', () => {
    expect(
      parseEngineRequest(
        '{"protocolVersion":1,"type":"request","id":"req-1","method":"engine.info"}',
      ),
    ).toEqual({
      protocolVersion: ENGINE_PROTOCOL_VERSION,
      type: 'request',
      id: 'req-1',
      method: 'engine.info',
    });
    expect(
      parseEngineRequest(
        '{"protocolVersion":1,"type":"request","id":"req-2","method":"engine.shutdown"}',
      ).method,
    ).toBe('engine.shutdown');
  });

  it('rejects malformed or unsupported requests at the boundary', () => {
    expect(() => parseEngineRequest('not json')).toThrow(ProtocolError);
    expect(() =>
      parseEngineRequest('{"protocolVersion":2,"type":"request","id":"a","method":"engine.info"}'),
    ).toThrow(/protocol version/);
    expect(() =>
      parseEngineRequest('{"protocolVersion":1,"type":"request","id":"","method":"engine.info"}'),
    ).toThrow(ProtocolError);
    expect(() =>
      parseEngineRequest('{"protocolVersion":1,"type":"request","id":"a","method":"audio.start"}'),
    ).toThrow(ProtocolError);
  });

  it('parses the startup auth message without exposing it as a control request', () => {
    expect(parseEngineAuth('{"protocolVersion":1,"type":"auth","token":"secret"}')).toEqual({
      protocolVersion: ENGINE_PROTOCOL_VERSION,
      type: 'auth',
      token: 'secret',
    });
    expect(() => parseEngineAuth('{"protocolVersion":1,"type":"auth","token":""}')).toThrow(
      ProtocolError,
    );
  });

  it('correlates responses with the originating request id', () => {
    const raw =
      '{"protocolVersion":1,"type":"response","id":"req-1","ok":true,"result":{"engineVersion":"0.0.0","nodeVersion":"v24.22.1","pid":123,"startedAt":"2026-11-16T00:00:00.000Z"}}';
    expect(parseEngineResponse(raw, 'req-1', 'engine.info').ok).toBe(true);
    expect(() => parseEngineResponse(raw, 'other-id', 'engine.info')).toThrow(/request id/);
  });

  it('validates response payloads against the requested method', () => {
    expect(
      parseEngineResponse(
        '{"protocolVersion":1,"type":"response","id":"stop","ok":true,"result":null}',
        'stop',
        'engine.shutdown',
      ),
    ).toMatchObject({ ok: true, result: null });
    expect(() =>
      parseEngineResponse(
        '{"protocolVersion":1,"type":"response","id":"info","ok":true,"result":null}',
        'info',
        'engine.info',
      ),
    ).toThrow(ProtocolError);
    expect(() =>
      parseEngineResponse(
        '{"protocolVersion":1,"type":"response","id":"stop","ok":true,"result":{"engineVersion":"0.0.0","nodeVersion":"v24.22.1","pid":1,"startedAt":"2026-11-16T00:00:00.000Z"}}',
        'stop',
        'engine.shutdown',
      ),
    ).toThrow(ProtocolError);
  });

  it('parses error responses and the dedicated startup handshake', () => {
    expect(
      parseEngineResponse(
        '{"protocolVersion":1,"type":"response","id":"req-1","ok":false,"error":{"code":"unauthorized","message":"authentication failed"}}',
        'req-1',
        'engine.info',
      ),
    ).toMatchObject({ ok: false, error: { code: 'unauthorized' } });
    expect(
      parseEngineHandshake(`${ENGINE_HANDSHAKE_PREFIX}{"protocolVersion":1,"port":4321,"pid":123}`),
    ).toEqual({ protocolVersion: ENGINE_PROTOCOL_VERSION, port: 4321, pid: 123 });
    expect(() => parseEngineHandshake('ordinary log line')).toThrow(ProtocolError);
  });
});
