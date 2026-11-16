import { Icon } from '@iconify/react';
import sunIcon from '@iconify-icons/lucide/sun';
import { Link, Outlet } from '@tanstack/react-router';
import { Button } from '@sonasub/ui/components/button';
import { useTheme, useToggleTheme } from '@sonasub/ui/theme';

export function AppLayout() {
  const theme = useTheme();
  const toggleTheme = useToggleTheme();
  let toggleLabel = 'Switch to dark';
  if (theme === 'dark') toggleLabel = 'Switch to light';

  return (
    <div className="min-h-screen bg-background text-foreground">
      <header className="flex items-center justify-between border-b border-border px-6 py-4">
        <div className="flex items-center gap-6">
          <span className="text-sm font-semibold tracking-wide">SonaSub</span>
          <nav className="flex items-center gap-4 text-sm">
            <Link to="/" activeProps={{ className: 'text-primary font-medium' }}>
              Overview
            </Link>
            <Link to="/about" activeProps={{ className: 'text-primary font-medium' }}>
              About
            </Link>
          </nav>
        </div>
        <Button variant="outline" size="sm" onClick={toggleTheme}>
          <Icon icon={sunIcon} className="size-4" />
          {toggleLabel}
        </Button>
      </header>
      <main className="mx-auto w-full max-w-3xl px-6 py-8">
        <Outlet />
      </main>
    </div>
  );
}
