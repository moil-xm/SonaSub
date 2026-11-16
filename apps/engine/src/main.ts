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

export function messageText(data: WebSocket.RawData): string {
  if (Array.isArray(data)) return Buffer.concat(data).toString('utf8');
  if (data instanceof ArrayBuffer) return Buffer.from(data).toString('utf8');
  return data.toString('utf8');
}

function response(id: string, result: EngineInfo | null): EngineResponse {
  return { protocolVersion: ENGINE_PROTOCOL_VERSION, type: 'response', id, ok: true, result };
}

function errorResponse(id: string, error: unknown): EngineResponse {
  let protocolError = new ProtocolError(
    'internal_error',
    'The engine failed to handle the request.',
  );
  if (error instanceof ProtocolError) protocolError = error;
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
    console.error(
      'The engine is missing the credential from the desktop Rust backend and refuses to start.',
    );
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
    socket.on('error', (error) => {
      console.error(`The engine client connection failed: ${error.message}`);
      socket.terminate();
    });
    if (shuttingDown || request.socket.remoteAddress !== '127.0.0.1') {
      socket.close(1008, 'Only the loopback desktop backend may connect.');
      return;
    }
    socket.on('message', (data, isBinary) => {
      if (isBinary) {
        socket.close(1008, 'The engine protocol accepts text messages only.');
        return;
      }
      const raw = messageText(data);
      if (!authenticated.has(socket)) {
        try {
          const auth = parseEngineAuth(raw);
          const candidate = Buffer.from(auth.token);
          if (
            candidate.length !== expectedToken.length ||
            !timingSafeEqual(candidate, expectedToken)
          ) {
            socket.close(1008, 'Engine connection authentication failed.');
            return;
          }
          authenticated.add(socket);
        } catch {
          socket.close(1008, 'Engine connection authentication failed.');
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
    console.error(`The engine loopback service failed: ${error.message}`);
    process.exit(1);
  });
  const address = server.address();
  if (address === null || typeof address === 'string') {
    throw new Error('The engine could not bind a loopback TCP port.');
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
  let message = 'The engine failed to start.';
  if (error instanceof Error) message = error.message;
  console.error(message);
  process.exit(1);
});
