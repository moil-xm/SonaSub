import dayjs from 'dayjs';

const MODULES = [
  ['apps/desktop', 'Desktop frontend and the src-tauri native backend'],
  ['apps/engine', 'Loopback WebSocket engine with real runtime information'],
  ['apps/extension', 'Chrome extension'],
  ['packages/ui', 'Shared components, theme, icons and styles'],
  ['packages/protocol', 'Desktop-backend and engine communication contract'],
];

export function AboutPage() {
  return (
    <div className="space-y-6">
      <p className="text-sm text-muted-foreground">
        SonaSub is an open-source real-time subtitling project. This page rendered at{' '}
        {dayjs().format('YYYY-MM-DD HH:mm')}.
      </p>
      <ul className="divide-y divide-border rounded-lg border border-border text-sm">
        {MODULES.map(([name, role]) => (
          <li key={name} className="flex items-baseline justify-between gap-4 px-4 py-3">
            <code className="font-mono text-xs">{name}</code>
            <span className="text-muted-foreground">{role}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
