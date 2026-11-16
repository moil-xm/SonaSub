import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { fileURLToPath } from 'node:url';

import { afterEach, describe, expect, it } from 'vite-plus/test';
import WebSocket from 'ws';

import {
  ENGINE_HANDSHAKE_PREFIX,
  parseEngineHandshake,
  parseEngineResponse,
} from '@sonasub/protocol/engine';

import { messageText } from './main';

const entry = fileURLToPath(new URL('./main.ts', import.meta.url));
const children = new Set<ReturnType<typeof spawn>>();

async function startEngine(token = 'test-token'): Promise<{
  port: number;
  pid: number;
  logs: string[];
  process: ReturnType<typeof spawn>;
}> {
  const child = spawn(process.execPath, [entry], {
    env: { ...process.env, SONASUB_ENGINE_TOKEN: token },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  children.add(child);
  const logs: string[] = [];
  const handshake = await new Promise<{ port: number; pid: number }>((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error('engine handshake timed out')), 5000);
    child.once('error', reject);
    child.once('exit', (code) => reject(new Error(`engine exited before handshake: ${code}`)));
    child.stdout.setEncoding('utf8');
    let buffer = '';
    child.stdout.on('data', (chunk: string) => {
      buffer += chunk;
      const lines = buffer.split('\n');
      buffer = lines.pop() ?? '';
      for (const line of lines) {
        logs.push(line);
        if (line.startsWith(ENGINE_HANDSHAKE_PREFIX)) {
          clearTimeout(timeout);
          resolve(parseEngineHandshake(line));
        }
      }
    });
  });
  return { ...handshake, logs, process: child };
}

async function openSocket(port: number): Promise<WebSocket> {
  const socket = new WebSocket(`ws://127.0.0.1:${port}`, { handshakeTimeout: 2000 });
  await once(socket, 'open');
  return socket;
}

function send(socket: WebSocket, message: object): void {
  socket.send(JSON.stringify(message));
}

async function response(socket: WebSocket, id: string, method: 'engine.info' | 'engine.shutdown') {
  const raw = await new Promise<string>((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error('response timed out')), 2000);
    socket.on('message', (data) => {
      const message = messageText(data);
      if (JSON.parse(message).id === id) {
        clearTimeout(timeout);
        resolve(message);
      }
    });
    socket.once('error', reject);
  });
  return parseEngineResponse(raw, id, method);
}

afterEach(async () => {
  await Promise.all(
    [...children].map(async (child) => {
      if (child.exitCode === null && child.signalCode === null) {
        const exited = once(child, 'exit');
        child.kill('SIGKILL');
        await exited;
      }
      children.delete(child);
    }),
  );
});

describe('real Node engine server', () => {
  it('rejects control before authentication and wrong credentials', async () => {
    const engine = await startEngine();
    const socket = await openSocket(engine.port);
    const closed = once(socket, 'close');
    send(socket, {
      protocolVersion: 1,
      type: 'request',
      id: 'before-auth',
      method: 'engine.info',
    });
    expect(await closed).toEqual([1008, expect.any(Buffer)]);
    const wrongSocket = await openSocket(engine.port);
    const wrongClosed = once(wrongSocket, 'close');
    send(wrongSocket, { protocolVersion: 1, type: 'auth', token: 'wrong' });
    expect(await wrongClosed).toEqual([1008, expect.any(Buffer)]);
    expect(engine.logs.join('\n')).not.toContain('test-token');
  });

  it('returns real process information and handles concurrent correlated requests', async () => {
    const engine = await startEngine();
    const socket = await openSocket(engine.port);
    send(socket, { protocolVersion: 1, type: 'auth', token: 'test-token' });
    const requests = ['first', 'second'].map((id) => {
      const result = response(socket, id, 'engine.info');
      send(socket, { protocolVersion: 1, type: 'request', id, method: 'engine.info' });
      return result;
    });
    const responses = await Promise.all(requests);
    for (const value of responses) {
      expect(value.ok).toBe(true);
      if (value.ok) {
        expect(value.result).toEqual({
          engineVersion: '0.0.0',
          nodeVersion: process.version,
          pid: engine.pid,
          startedAt: expect.any(String),
        });
      }
    }
    expect(responses.map((value) => value.id)).toEqual(['first', 'second']);
    socket.close();
  });

  it('acknowledges an authenticated shutdown and exits without leaving a socket', async () => {
    const engine = await startEngine();
    const socket = await openSocket(engine.port);
    const exited = once(engine.process, 'exit');
    send(socket, { protocolVersion: 1, type: 'auth', token: 'test-token' });
    send(socket, {
      protocolVersion: 1,
      type: 'request',
      id: 'shutdown',
      method: 'engine.shutdown',
    });
    expect(await response(socket, 'shutdown', 'engine.shutdown')).toMatchObject({
      ok: true,
      result: null,
    });
    expect(await exited).toEqual([0, null]);
    children.delete(engine.process);
    await expect(openSocket(engine.port)).rejects.toThrow();
  });

  it('requires Rust-owned authentication and binds an OS-selected loopback port', async () => {
    const child = spawn(process.execPath, [entry], {
      env: { ...process.env, SONASUB_ENGINE_TOKEN: '' },
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    const [code] = await once(child, 'exit');
    expect(code).not.toBe(0);
    const engine = await startEngine();
    expect(engine.port).toBeGreaterThan(0);
    expect(engine.pid).toBe(engine.process.pid);
  });
});
