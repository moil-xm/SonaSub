import dayjs from 'dayjs';

const modules = [
  ['apps/desktop', '桌面前端与 src-tauri 原生后端'],
  ['apps/engine', '本机 WebSocket 引擎与真实运行信息'],
  ['apps/extension', 'Chrome 扩展'],
  ['packages/ui', '共享组件、主题、图标与样式'],
  ['packages/protocol', '桌面后端与引擎的通信契约'],
];

export function AboutPage() {
  return (
    <div className="space-y-6">
      <p className="text-sm text-muted-foreground">
        SonaSub 是开源实时字幕项目。本页面渲染于 {dayjs().format('YYYY-MM-DD HH:mm')}。
      </p>
      <ul className="divide-y divide-border rounded-lg border border-border text-sm">
        {modules.map(([name, role]) => (
          <li key={name} className="flex items-baseline justify-between gap-4 px-4 py-3">
            <code className="font-mono text-xs">{name}</code>
            <span className="text-muted-foreground">{role}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
