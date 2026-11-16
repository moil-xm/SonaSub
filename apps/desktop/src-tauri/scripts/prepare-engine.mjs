import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdir, mkdtemp, readFile, rename, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const manifestDirectory = resolve(scriptDirectory, '..');
const workspace = resolve(manifestDirectory, '../../..');
const resources = join(manifestDirectory, 'resources');
const runtimeDirectory = join(resources, 'runtime');
const manifest = JSON.parse(await readFile(join(scriptDirectory, 'node-runtime.json'), 'utf8'));
const platform = `${process.platform}-${process.arch}`;
const runtime = manifest.platforms[platform];

if (!runtime) {
  throw new Error(`没有为 ${platform} 配置 Node.js 运行时。`);
}

const nodeBinary = join(runtimeDirectory, runtime.executable);
const versionMarker = join(runtimeDirectory, '.node-version');
if (
  existsSync(nodeBinary) &&
  existsSync(versionMarker) &&
  (await readFile(versionMarker, 'utf8')).trim() === `${manifest.version}-${platform}`
) {
  console.info(`Node.js ${manifest.version} (${platform}) 已就绪。`);
} else {
  const temporary = await mkdtemp(join(tmpdir(), 'sonasub-node-'));
  try {
    const archive = join(temporary, runtime.archive);
    const url = `https://nodejs.org/dist/v${manifest.version}/${runtime.archive}`;
    const response = await fetch(url, { signal: AbortSignal.timeout(120_000) });
    if (!response.ok) {
      throw new Error(`下载 Node.js 失败：HTTP ${response.status}`);
    }
    const bytes = Buffer.from(await response.arrayBuffer());
    const hash = createHash('sha256').update(bytes).digest('hex');
    if (hash !== runtime.sha256) {
      throw new Error(`Node.js 校验和不匹配：期望 ${runtime.sha256}，实际 ${hash}`);
    }
    await writeFile(archive, bytes);
    const extracted = join(temporary, 'extracted');
    await mkdir(extracted);
    if (runtime.archive.endsWith('.zip')) {
      execFileSync('unzip', ['-q', archive, '-d', extracted], { stdio: 'inherit' });
    } else {
      execFileSync('tar', ['-xzf', archive, '-C', extracted], { stdio: 'inherit' });
    }
    const archiveRoot = join(extracted, runtime.archive.replace(/\.(tar\.gz|zip)$/, ''));
    await rm(runtimeDirectory, { recursive: true, force: true });
    await mkdir(resources, { recursive: true });
    await rename(archiveRoot, runtimeDirectory);
    await writeFile(versionMarker, `${manifest.version}-${platform}\n`);
    console.info(`已安装并校验 Node.js ${manifest.version} (${platform})。`);
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}

execFileSync('pnpm', ['--filter', '@sonasub/engine', 'build'], {
  cwd: workspace,
  stdio: 'inherit',
});
const engineDirectory = join(resources, 'engine');
await rm(engineDirectory, { recursive: true, force: true });
await mkdir(engineDirectory, { recursive: true });
await rename(join(workspace, 'apps/engine/dist/main.mjs'), join(engineDirectory, 'main.mjs'));
console.info(`引擎产物已写入 ${engineDirectory}/main.mjs。`);
