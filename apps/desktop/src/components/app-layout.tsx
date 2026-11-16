import { Icon } from '@iconify/react';
import sunIcon from '@iconify-icons/lucide/sun';
import { Link, Outlet } from '@tanstack/react-router';
import { Button } from '@sonasub/ui/components/button';
import { useThemeStore } from '@sonasub/ui/theme';

export function AppLayout() {
  const theme = useThemeStore((state) => state.theme);
  const toggleTheme = useThemeStore((state) => state.toggleTheme);

  return (
    <div className="min-h-screen bg-background text-foreground">
      <header className="flex items-center justify-between border-b border-border px-6 py-4">
        <div className="flex items-center gap-6">
          <span className="text-sm font-semibold tracking-wide">SonaSub</span>
          <nav className="flex items-center gap-4 text-sm">
            <Link to="/" activeProps={{ className: 'text-primary font-medium' }}>
              概览
            </Link>
            <Link to="/about" activeProps={{ className: 'text-primary font-medium' }}>
              关于
            </Link>
          </nav>
        </div>
        <Button variant="outline" size="sm" onClick={toggleTheme}>
          <Icon icon={sunIcon} className="size-4" />
          {theme === 'dark' ? '切换到浅色' : '切换到深色'}
        </Button>
      </header>
      <main className="mx-auto w-full max-w-3xl px-6 py-8">
        <Outlet />
      </main>
    </div>
  );
}
