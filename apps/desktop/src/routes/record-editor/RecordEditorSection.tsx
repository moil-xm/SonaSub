import type { ReactNode } from 'react';

type RecordEditorSectionProps = {
  children: ReactNode;
  index: number;
  optional?: string;
  title: string;
};

export function RecordEditorSection({
  children,
  index,
  optional,
  title,
}: RecordEditorSectionProps) {
  return (
    <section className="overflow-hidden rounded-xl border bg-card">
      <header className="flex items-center gap-3 px-4 py-3">
        <span className="grid size-5 place-items-center rounded-full bg-muted font-mono text-[10px] font-semibold text-muted-foreground">
          {String(index).padStart(2, '0')}
        </span>
        <h2 className="min-w-0 flex-1 text-[13px] font-semibold">{title}</h2>
        {optional && (
          <span className="rounded-full bg-muted px-2 py-0.5 text-[10px] text-muted-foreground">
            {optional}
          </span>
        )}
      </header>
      <div className="border-t border-border/70 p-4">{children}</div>
    </section>
  );
}
