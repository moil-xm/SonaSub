import { timingSafeEqual } from 'node:crypto';
import { createServer } from 'node:http';

import {
  ENGINE_HANDSHAKE_PREFIX,
  ENGINE_PROTOCOL_VERSION,
  ProtocolError,
  parseEngineAuth,
  parseEngineRequest,
} from '@sonasub/protocol/engine';
import type { EngineInfo, EngineResponse } from '@sonasub/protocol/engine';
import { WebSocketServer, type WebSocket } from 'ws';

import enginePackage from '../package.json' with { type: 'json' };

const startedAt = new Date().toISOString();

function response(id: string, result: EngineInfo | null): EngineResponse {
  return { protocolVersion: ENGINE_PROTOCOL_VERSION, type: 'response', id, ok: true, result };
}

function errorResponse(id: string, error: unknown): EngineResponse {
  const protocolError =
    error instanceof ProtocolError
      ? error
      : new ProtocolError('internal_error', '引擎处理请求失败。');
  return {
    protocolVersion: ENGINE_PROTOCOL_VERSION,
    type: 'response',
    id,
    ok: false,
    error: { code: protocolError.code, message: protocolError.message },
  };
}

async function main(): Promise<void> {
  const token = process.env.SONASUB_ENGINE_TOKEN;
  if (!token) {
    console.error('引擎缺少由桌面 Rust 后端提供的连接认证信息，拒绝启动。');
    process.exitCode = 1;
    return;
  }
  const expectedToken = Buffer.from(token);
  const server = createServer((_request, reply) => {
    reply.writeHead(426, { Connection: 'close' });
    reply.end();
  });
  const sockets = new WebSocketServer({ server, maxPayload: 8192 });
  const authenticated = new WeakSet<WebSocket>();
  let shuttingDown = false;

  const shutdown = () => {
    if (shuttingDown) return;
    shuttingDown = true;
    sockets.close();
    server.close(() => {
      process.exitCode = 0;
    });
    server.closeAllConnections();
    setTimeout(() => process.exit(0), 1000).unref();
  };

  sockets.on('connection', (socket, request) => {
    if (shuttingDown || request.socket.remoteAddress !== '127.0.0.1') {
      socket.close(1008, '仅接受本机桌面后端连接。');
      return;
    }
    socket.on('message', (data, isBinary) => {
      if (isBinary) {
        socket.close(1008, '引擎协议只接受文本消息。');
        return;
      }
      const raw = Array.isArray(data)
        ? Buffer.concat(data).toString('utf8')
        : data instanceof ArrayBuffer
          ? Buffer.from(data).toString('utf8')
          : data.toString('utf8');
      if (!authenticated.has(socket)) {
        try {
          const auth = parseEngineAuth(raw);
          const candidate = Buffer.from(auth.token);
          if (
            candidate.length !== expectedToken.length ||
            !timingSafeEqual(candidate, expectedToken)
          ) {
            socket.close(1008, '引擎连接认证失败。');
            return;
          }
          authenticated.add(socket);
        } catch {
          socket.close(1008, '引擎连接认证失败。');
        }
        return;
      }
      let id = 'invalid';
      try {
        const message = parseEngineRequest(raw);
        id = message.id;
        if (message.method === 'engine.info') {
          socket.send(
            JSON.stringify(
              response(id, {
                engineVersion: enginePackage.version,
                nodeVersion: process.version,
                pid: process.pid,
                startedAt,
              }),
            ),
          );
          return;
        }
        socket.send(JSON.stringify(response(id, null)), () => shutdown());
      } catch (error) {
        socket.send(JSON.stringify(errorResponse(id, error)));
      }
    });
  });

  await new Promise<void>((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  server.on('error', (error) => {
    console.error(`引擎本机服务失败：${error.message}`);
    process.exit(1);
  });
  const address = server.address();
  if (address === null || typeof address === 'string') {
    throw new Error('引擎未能绑定本机 TCP 端口。');
  }
  process.stdout.write(
    `${ENGINE_HANDSHAKE_PREFIX}${JSON.stringify({
      protocolVersion: ENGINE_PROTOCOL_VERSION,
      port: address.port,
      pid: process.pid,
    })}\n`,
  );
  process.once('SIGTERM', shutdown);
  process.once('SIGINT', shutdown);
}

main().catch((error: unknown) => {
  console.error(error instanceof Error ? error.message : '引擎启动失败。');
  process.exit(1);
});
