#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { createReadStream } from 'node:fs';
import { copyFile, mkdir, mkdtemp, rename, rm, stat } from 'node:fs/promises';
import { homedir, tmpdir } from 'node:os';
import path from 'node:path';

const MODEL_ROOT = process.env.SONASUB_MODELS_DIR
  ? path.resolve(process.env.SONASUB_MODELS_DIR)
  : path.join(homedir(), 'Library', 'Application Support', 'dev.sonasub.desktop', 'models');
const CACHE_ROOT = path.join(MODEL_ROOT, '.cache');

const SHERPA_RELEASE = 'https://github.com/k2-fsa/sherpa-onnx/releases/download';
const MODELSCOPE_API = 'https://modelscope.cn/api/v1/models/zengshuishui/Qwen3-ASR-onnx/repo';
const QWEN3_17B_REVISION = '9c182309f7bb075f241424441add9e16c5086dfb';
const FIREFOX_ATTACHMENTS =
  'https://firefox-settings-attachments.cdn.mozilla.net/main-workspace/translations-models-v2';

const downloads = {
  streaming: {
    url: `${SHERPA_RELEASE}/asr-models/sherpa-onnx-streaming-zipformer-bilingual-zh-en-2023-02-20.tar.bz2`,
    file: 'streaming.tar.bz2',
    bytes: 511274346,
  },
  senseVoice: {
    url: `${SHERPA_RELEASE}/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17.tar.bz2`,
    file: 'sense-voice.tar.bz2',
    bytes: 163002883,
  },
  qwen3_06b: {
    url: `${SHERPA_RELEASE}/asr-models/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25.tar.bz2`,
    file: 'qwen3-asr-0.6b.tar.bz2',
    bytes: 878702423,
    sha256: '393f8a14e2f5fb96746aaab342997a40641001fbd5bf9592a080a8329178ee96',
  },
  vad: {
    url: `${SHERPA_RELEASE}/asr-models/silero_vad.onnx`,
    file: 'silero_vad.onnx',
    bytes: 643854,
  },
  pyannote: {
    url: `${SHERPA_RELEASE}/speaker-segmentation-models/sherpa-onnx-pyannote-segmentation-3-0.tar.bz2`,
    file: 'pyannote.tar.bz2',
    bytes: 6958444,
  },
  speakerEmbedding: {
    url: `${SHERPA_RELEASE}/speaker-recongition-models/3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx`,
    file: 'campplus.onnx',
    bytes: 28281164,
  },
};

const qwen3_06b = {
  root: 'sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25',
  files: [
    {
      source: 'conv_frontend.onnx',
      target: 'sherpa/qwen3-asr-0.6b-int8/conv_frontend.onnx',
      sha256: 'd22dc4423e0940e49884e903d2ea2f7e5567c14fc1aed97e4e26d6b8f208ef9e',
    },
    {
      source: 'encoder.int8.onnx',
      target: 'sherpa/qwen3-asr-0.6b-int8/encoder.int8.onnx',
      sha256: '60748d3e6744a57c9c91e1b17424a6c2990567e8adceb0783940c03ed98fa9d9',
    },
    {
      source: 'decoder.int8.onnx',
      target: 'sherpa/qwen3-asr-0.6b-int8/decoder.int8.onnx',
      sha256: '4f6885be5959ae26af3089d38ee7972c5fafbeeb1cf8d5e76eab6d8b61ca5771',
    },
    {
      source: 'tokenizer/merges.txt',
      target: 'sherpa/qwen3-asr-0.6b-int8/tokenizer/merges.txt',
      sha256: '8831e4f1a044471340f7c0a83d7bd71306a5b867e95fd870f74d0c5308a904d5',
    },
    {
      source: 'tokenizer/tokenizer_config.json',
      target: 'sherpa/qwen3-asr-0.6b-int8/tokenizer/tokenizer_config.json',
      sha256: '4942d005604266809309cabc9f4e9cb89ce855d59b14681fdc0e1cc62ea26c4c',
    },
    {
      source: 'tokenizer/vocab.json',
      target: 'sherpa/qwen3-asr-0.6b-int8/tokenizer/vocab.json',
      sha256: 'ca10d7e9fb3ed18575dd1e277a2579c16d108e32f27439684afa0e10b1440910',
    },
  ],
};

const qwen3_17b = [
  {
    source: 'model_1.7B/conv_frontend.onnx',
    file: 'qwen3-17b-conv_frontend.onnx',
    target: 'sherpa/qwen3-asr-1.7b-int8/conv_frontend.onnx',
    bytes: 48080441,
    sha256: 'fa894a4ba53da6a4238f2a6ca0b09362e505d39cecbd646051b033e2e8d7e2fb',
  },
  {
    source: 'model_1.7B/encoder.int8.onnx',
    file: 'qwen3-17b-encoder.int8.onnx',
    target: 'sherpa/qwen3-asr-1.7b-int8/encoder.int8.onnx',
    bytes: 314222162,
    sha256: '436fbd910a0c8914851e5ac1354e807be9f283d08a5da728adaa609731c41469',
  },
  {
    source: 'model_1.7B/decoder.int8.onnx',
    file: 'qwen3-17b-decoder.int8.onnx',
    target: 'sherpa/qwen3-asr-1.7b-int8/decoder.int8.onnx',
    bytes: 2037458645,
    sha256: 'c43c853fa6e97d08365cb8a5502b360b595cd43c00dc60e4d8ca7cc18cad460b',
  },
  {
    source: 'tokenizer/merges.txt',
    file: 'qwen3-17b-merges.txt',
    target: 'sherpa/qwen3-asr-1.7b-int8/tokenizer/merges.txt',
    bytes: 1671853,
    sha256: '8831e4f1a044471340f7c0a83d7bd71306a5b867e95fd870f74d0c5308a904d5',
  },
  {
    source: 'tokenizer/tokenizer_config.json',
    file: 'qwen3-17b-tokenizer_config.json',
    target: 'sherpa/qwen3-asr-1.7b-int8/tokenizer/tokenizer_config.json',
    bytes: 12487,
    sha256: '4942d005604266809309cabc9f4e9cb89ce855d59b14681fdc0e1cc62ea26c4c',
  },
  {
    source: 'tokenizer/vocab.json',
    file: 'qwen3-17b-vocab.json',
    target: 'sherpa/qwen3-asr-1.7b-int8/tokenizer/vocab.json',
    bytes: 2776833,
    sha256: 'ca10d7e9fb3ed18575dd1e277a2579c16d108e32f27439684afa0e10b1440910',
  },
].map((file) => ({
  ...file,
  url: `${MODELSCOPE_API}?Revision=${QWEN3_17B_REVISION}&FilePath=${encodeURIComponent(file.source)}`,
}));

const bergamot = [
  {
    url: `${FIREFOX_ATTACHMENTS}/321191b6-2c42-4bca-84ec-b7b074b67bd6.zst`,
    file: 'model.enzh.intgemm.alphas.bin.zst',
    bytes: 32923790,
    target: 'bergamot/en-zh-Hans/model.bin',
    sha256: '4e5accc141373565ddc8fa1565bceaa8d0c3482a82cab8131c719ebcc6c2157c',
  },
  {
    url: `${FIREFOX_ATTACHMENTS}/83278466-fec4-4944-a99d-c2372fa9de88.zst`,
    file: 'srcvocab.enzh.spm.zst',
    bytes: 348078,
    target: 'bergamot/en-zh-Hans/src.spm',
    sha256: 'bd9b65504acc6d9726dd281f7defc2adb7c2c22d0688fe2f84697de25197c8c5',
  },
  {
    url: `${FIREFOX_ATTACHMENTS}/84236654-1b66-42fc-9ba2-74c37f27e5eb.zst`,
    file: 'trgvocab.enzh.spm.zst',
    bytes: 354994,
    target: 'bergamot/en-zh-Hans/trg.spm',
    sha256: 'aded6993c36e440284d11cec3f6b8aef9c0e43188a772d80be342a713adf223d',
  },
  {
    url: `${FIREFOX_ATTACHMENTS}/900beaed-2dba-4a88-ad7c-852ec9e73f6a.zst`,
    file: 'lex.50.50.enzh.s2t.bin.zst',
    bytes: 2499275,
    target: 'bergamot/en-zh-Hans/lex.bin',
    sha256: '4a5e5827788060f1d718a8132b69440929387514a045796e9b77f935db68c055',
  },
  {
    url: `${FIREFOX_ATTACHMENTS}/8bb74789-fad9-40e0-8f49-d3e017106138.zst`,
    file: 'model.zhen.intgemm.alphas.bin.zst',
    bytes: 32186434,
    target: 'bergamot/zh-Hans-en/model.bin',
    sha256: '5cd149601802fc8a18124a1c1306144dbbedc058630c4ddb2d53aa76fa9c7c06',
  },
  {
    url: `${FIREFOX_ATTACHMENTS}/369efa59-d98e-4aa9-8a91-363ba16c3ba7.zst`,
    file: 'vocab.zhen.spm.zst',
    bytes: 611595,
    target: 'bergamot/zh-Hans-en/vocab.spm',
    sha256: 'dff594318ab7d8b7b60b844ab98ebe6b932ae8045fab15235404c787715965b3',
  },
  {
    url: `${FIREFOX_ATTACHMENTS}/a2261cbe-7fe7-498d-a128-c08d918b42fc.zst`,
    file: 'lex.50.50.zhen.s2t.bin.zst',
    bytes: 3427789,
    target: 'bergamot/zh-Hans-en/lex.bin',
    sha256: '8524dd4c93ffd7f0ae7be32f77a7c14fea7cdbbebe34ea6ad0ec368d5ddd8b13',
  },
];

const expected = {
  streaming: [
    [
      'sherpa/streaming-zipformer-bilingual-zh-en/encoder.onnx',
      '8fa764187a261844f859d7143ebaa563af5d10adfece4c18a8f414c88cba2a9b',
    ],
    [
      'sherpa/streaming-zipformer-bilingual-zh-en/decoder.onnx',
      '1a70c593d71e53f023f5f55b0b4cfff5055abb786ee3992e5f63dc2e273cc4fa',
    ],
    [
      'sherpa/streaming-zipformer-bilingual-zh-en/joiner.onnx',
      '1ed689c5ed19dbaa725d9d191bb4822b5f4855a39e1ffd28cbc1f340d25b2ee0',
    ],
    [
      'sherpa/streaming-zipformer-bilingual-zh-en/tokens.txt',
      'a8e0e4ec53810e433789b54a5c0134a7eaa2ffca595a6334d54c00da858841d3',
    ],
  ],
  senseVoice: [
    [
      'sherpa/sense-voice-int8/model.onnx',
      'c71f0ce00bec95b07744e116345e33d8cbbe08cef896382cf907bf4b51a2cd51',
    ],
    [
      'sherpa/sense-voice-int8/tokens.txt',
      'f449eb28dc567533d7fa59be34e2abca8784f771850c78a47fb731a31429a1dc',
    ],
  ],
  qwen3_06b: qwen3_06b.files.map((file) => [file.target, file.sha256]),
  qwen3_17b: qwen3_17b.map((file) => [file.target, file.sha256]),
  vad: [
    ['sherpa/silero_vad.onnx', '9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6'],
  ],
  speakerEmbedding: [
    [
      'sherpa/speaker/campplus-zh-en.onnx',
      'aa3cfc16963a10586a9393f5035d6d6b57e98d358b347f80c2a30bf4f00ceba2',
    ],
  ],
  diarization: [
    [
      'sherpa/diarization/pyannote-segmentation-3.0.onnx',
      'd582f4b4c6b48205de7e0643c57df0df5615a3c176189be3fc461e9d18827b5d',
    ],
  ],
  bergamot: bergamot.map((file) => [file.target, file.sha256]),
};

const expectedFiles = Object.values(expected).flat();

function run(command, args) {
  const result = spawnSync(command, args, { stdio: 'inherit' });
  if (result.error) throw new Error(`${command} failed: ${result.error.message}`);
  if (result.status !== 0) throw new Error(`${command} exited with status ${result.status}`);
}

async function fileSize(file) {
  try {
    const metadata = await stat(file);
    return metadata.isFile() ? metadata.size : null;
  } catch {
    return null;
  }
}

async function sha256(file) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(file)) hash.update(chunk);
  return hash.digest('hex');
}

async function isExpected(file, checksum) {
  const size = await fileSize(file);
  if (size === null) return false;
  return (await sha256(file)) === checksum;
}

async function allExpected(files) {
  for (const [relative, checksum] of files) {
    if (!(await isExpected(path.join(MODEL_ROOT, relative), checksum))) return false;
  }
  return true;
}

function curl(url, output) {
  return spawnSync(
    '/usr/bin/curl',
    [
      '--fail',
      '--location',
      '--retry',
      '5',
      '--retry-all-errors',
      '--retry-delay',
      '2',
      '--connect-timeout',
      '30',
      '--continue-at',
      '-',
      '--output',
      output,
      url,
    ],
    { stdio: 'inherit' },
  );
}

async function download(asset) {
  const cache = path.join(CACHE_ROOT, asset.file);
  const cached = await fileSize(cache);
  if (cached === asset.bytes) {
    if (!asset.sha256 || (await sha256(cache)) === asset.sha256) return cache;
    console.info(`Discarding cached ${asset.file} after its checksum did not match.`);
    await rm(cache, { force: true });
  }
  if (cached !== null) {
    console.info(`Discarding cached ${asset.file} (${cached} bytes, expected ${asset.bytes}).`);
    await rm(cache, { force: true });
  }
  await mkdir(CACHE_ROOT, { recursive: true });
  const partial = `${cache}.part`;
  let partialSize = await fileSize(partial);
  if (partialSize !== null && partialSize > asset.bytes) {
    await rm(partial, { force: true });
    partialSize = null;
  }
  console.info(
    `Downloading ${asset.file}${partialSize ? ` (resuming from ${partialSize} bytes)` : ''}...`,
  );
  let result = curl(asset.url, partial);
  if (result.error) throw new Error(`curl failed: ${result.error.message}`);
  if (result.status !== 0) {
    const size = await fileSize(partial);
    if (size === asset.bytes) {
      await rename(partial, cache);
      return cache;
    }
    if (partialSize !== null && (result.status === 22 || result.status === 33)) {
      console.info('Resume was rejected; restarting this download from zero.');
      await rm(partial, { force: true });
      result = curl(asset.url, partial);
    }
  }
  if (result.error) throw new Error(`curl failed: ${result.error.message}`);
  if (result.status !== 0) {
    const size = await fileSize(partial);
    throw new Error(
      `curl exited with status ${result.status} for ${asset.file} (${size ?? 0} of ${asset.bytes} bytes cached; rerun to resume)`,
    );
  }
  const size = await fileSize(partial);
  if (size !== asset.bytes) {
    throw new Error(
      `Downloaded ${asset.file} is ${size ?? 0} bytes, expected ${asset.bytes}; rerun to retry`,
    );
  }
  if (asset.sha256 && (await sha256(partial)) !== asset.sha256) {
    await rm(partial, { force: true });
    throw new Error(
      `Checksum mismatch for ${asset.file}: expected ${asset.sha256}. The partial download was discarded.`,
    );
  }
  await rename(partial, cache);
  return cache;
}

async function extract(archive, directory) {
  await mkdir(directory, { recursive: true });
  run('tar', ['-xjf', archive, '-C', directory]);
}

async function installFile(source, target, checksum, cache) {
  if (await isExpected(target, checksum)) return;
  await mkdir(path.dirname(target), { recursive: true });
  const temporary = `${target}.tmp-${process.pid}`;
  await copyFile(source, temporary);
  const actual = await sha256(temporary);
  if (actual !== checksum) {
    await rm(temporary, { force: true });
    if (cache) await rm(cache, { force: true });
    throw new Error(
      `Checksum mismatch for ${target}: expected ${checksum}, got ${actual}. The cached archive was discarded; rerun to download it again.`,
    );
  }
  await rename(temporary, target);
  console.info(`Installed ${path.relative(MODEL_ROOT, target)}`);
}

async function installZstd(archive, target, checksum) {
  if (await isExpected(target, checksum)) return;
  await mkdir(path.dirname(target), { recursive: true });
  const temporary = `${target}.tmp-${process.pid}`;
  run('zstd', ['-d', '-q', '-f', archive, '-o', temporary]);
  const actual = await sha256(temporary);
  if (actual !== checksum) {
    await rm(temporary, { force: true });
    await rm(archive, { force: true });
    throw new Error(
      `Checksum mismatch for ${target}: expected ${checksum}, got ${actual}. The cached archive was discarded; rerun to download it again.`,
    );
  }
  await rename(temporary, target);
  console.info(`Installed ${path.relative(MODEL_ROOT, target)}`);
}

async function prepareStreaming(temporary) {
  if (await allExpected(expected.streaming)) return;
  const archive = await download(downloads.streaming);
  const extracted = path.join(temporary, 'streaming');
  await extract(archive, extracted);
  const root = path.join(extracted, 'sherpa-onnx-streaming-zipformer-bilingual-zh-en-2023-02-20');
  await installFile(
    path.join(root, 'encoder-epoch-99-avg-1.int8.onnx'),
    path.join(MODEL_ROOT, expected.streaming[0][0]),
    expected.streaming[0][1],
    archive,
  );
  await installFile(
    path.join(root, 'decoder-epoch-99-avg-1.int8.onnx'),
    path.join(MODEL_ROOT, expected.streaming[1][0]),
    expected.streaming[1][1],
    archive,
  );
  await installFile(
    path.join(root, 'joiner-epoch-99-avg-1.int8.onnx'),
    path.join(MODEL_ROOT, expected.streaming[2][0]),
    expected.streaming[2][1],
    archive,
  );
  await installFile(
    path.join(root, 'tokens.txt'),
    path.join(MODEL_ROOT, expected.streaming[3][0]),
    expected.streaming[3][1],
    archive,
  );
}

async function prepareSenseVoice(temporary) {
  if (await allExpected(expected.senseVoice)) return;
  const archive = await download(downloads.senseVoice);
  const extracted = path.join(temporary, 'sense-voice');
  await extract(archive, extracted);
  const root = path.join(extracted, 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17');
  await installFile(
    path.join(root, 'model.int8.onnx'),
    path.join(MODEL_ROOT, expected.senseVoice[0][0]),
    expected.senseVoice[0][1],
    archive,
  );
  await installFile(
    path.join(root, 'tokens.txt'),
    path.join(MODEL_ROOT, expected.senseVoice[1][0]),
    expected.senseVoice[1][1],
    archive,
  );
}

async function prepareQwen3_06b(temporary) {
  if (await allExpected(expected.qwen3_06b)) return;
  const archive = await download(downloads.qwen3_06b);
  const extracted = path.join(temporary, 'qwen3-06b');
  await extract(archive, extracted);
  const root = path.join(extracted, qwen3_06b.root);
  for (const file of qwen3_06b.files) {
    await installFile(
      path.join(root, file.source),
      path.join(MODEL_ROOT, file.target),
      file.sha256,
      archive,
    );
  }
}

async function prepareQwen3_17b() {
  for (const file of qwen3_17b) {
    const target = path.join(MODEL_ROOT, file.target);
    if (await isExpected(target, file.sha256)) continue;
    const cached = await download(file);
    await installFile(cached, target, file.sha256, cached);
  }
}

async function prepareVad() {
  if (await allExpected(expected.vad)) return;
  const cached = await download(downloads.vad);
  await installFile(cached, path.join(MODEL_ROOT, expected.vad[0][0]), expected.vad[0][1], cached);
}

async function prepareSpeakerEmbedding() {
  if (await allExpected(expected.speakerEmbedding)) return;
  const cached = await download(downloads.speakerEmbedding);
  await installFile(
    cached,
    path.join(MODEL_ROOT, expected.speakerEmbedding[0][0]),
    expected.speakerEmbedding[0][1],
    cached,
  );
}

async function prepareDiarization(temporary) {
  if (await allExpected(expected.diarization)) return;
  const archive = await download(downloads.pyannote);
  const extracted = path.join(temporary, 'pyannote');
  await extract(archive, extracted);
  await installFile(
    path.join(extracted, 'sherpa-onnx-pyannote-segmentation-3-0', 'model.int8.onnx'),
    path.join(MODEL_ROOT, expected.diarization[0][0]),
    expected.diarization[0][1],
    archive,
  );
}

async function prepareBergamot() {
  for (const file of bergamot) {
    const target = path.join(MODEL_ROOT, file.target);
    if (await isExpected(target, file.sha256)) continue;
    const cached = await download(file);
    await installZstd(cached, target, file.sha256);
  }
}

async function main() {
  if (process.platform !== 'darwin') {
    throw new Error(
      'The default model layout follows the macOS application data directory. Set SONASUB_MODELS_DIR to prepare models elsewhere.',
    );
  }
  await mkdir(MODEL_ROOT, { recursive: true });
  const temporary = await mkdtemp(path.join(tmpdir(), 'sonasub-models-'));
  try {
    await prepareStreaming(temporary);
    await prepareSenseVoice(temporary);
    await prepareQwen3_06b(temporary);
    await prepareQwen3_17b();
    await prepareVad();
    await prepareSpeakerEmbedding();
    await prepareDiarization(temporary);
    await prepareBergamot();
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }

  const failures = [];
  for (const [relative, checksum] of expectedFiles) {
    const file = path.join(MODEL_ROOT, relative);
    if (!(await isExpected(file, checksum))) failures.push(relative);
  }
  if (failures.length > 0) {
    throw new Error(`Checksum verification failed for:\n${failures.join('\n')}`);
  }
  console.info(`All pinned models are ready in ${MODEL_ROOT}`);
  console.info(`Downloaded archives remain cached in ${CACHE_ROOT} for future repairs.`);
}

main().catch((error) => {
  console.error(error.message);
  console.error(`Successful downloads are cached in ${CACHE_ROOT}; rerun to resume.`);
  process.exitCode = 1;
});
