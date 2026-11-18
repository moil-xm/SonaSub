import { useEffect, useRef, useState } from 'react';
import { Icon } from '@iconify/react';
import activityIcon from '@iconify-icons/lucide/activity';
import captionsIcon from '@iconify-icons/lucide/captions';
import moonIcon from '@iconify-icons/lucide/moon';
import packageIcon from '@iconify-icons/lucide/package';
import settingsIcon from '@iconify-icons/lucide/settings';
import slidersHorizontalIcon from '@iconify-icons/lucide/sliders-horizontal';
import sunIcon from '@iconify-icons/lucide/sun';
import { useTranslation } from '@sonasub/i18n';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@sonasub/ui/components/alert-dialog';
import { useTheme, useToggleTheme } from '@sonasub/ui/theme';
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useQuery } from '@tanstack/react-query';
import { Link, Outlet, useRouterState } from '@tanstack/react-router';

import { RecordNav } from '@/components/record-nav';
import { recordsOptions, useRecordEvents } from '@/lib/records';
import { sessionSnapshotOptions, useSessionEvents } from '@/lib/session';
import { useConfirmBeforeQuit } from '@/stores/app-preferences';

function navigationItemClass(isActive: boolean) {
  if (isActive) {
    return 'flex h-[30px] items-center gap-2.5 rounded-[7px] bg-card px-2.5 text-[13px] font-medium text-foreground shadow-sm';
  }
  return 'flex h-[30px] items-center gap-2.5 rounded-[7px] px-2.5 text-[13px] text-muted-foreground transition-colors hover:bg-muted hover:text-foreground';
}

function settingsSubItemClass(isActive: boolean) {
  if (isActive) {
    return 'flex h-[26px] items-center rounded-md px-2.5 text-xs font-medium text-foreground';
  }
  return 'flex h-[26px] items-center rounded-md px-2.5 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground';
}

function currentPage(isActive: boolean): 'page' | undefined {
  if (isActive) return 'page';
  return undefined;
}

export function AppLayout() {
  const { t } = useTranslation();
  const location = useRouterState({ select: (state) => state.location });
  const pathname = location.pathname;
  const searchRecord = location.search.record;
  let activeRecordId: string | null = null;
  if (typeof searchRecord === 'string') activeRecordId = searchRecord;
  const { data: snapshot } = useQuery(sessionSnapshotOptions);
  const { data: records } = useQuery(recordsOptions);
  const theme = useTheme();
  const toggleTheme = useToggleTheme();
  const confirmBeforeQuit = useConfirmBeforeQuit();
  const [isQuitDialogOpen, setIsQuitDialogOpen] = useState(false);
  const allowCloseRef = useRef(false);
  useSessionEvents();
  useRecordEvents();

  const isLivePage = pathname === '/';
  const isRecordsPage = pathname === '/' || pathname.startsWith('/records/');
  const isNewRecordPage = pathname === '/records/new';
  const isEditRecordPage = pathname.endsWith('/edit');
  const isSessionPage = pathname === '/session';
  const isDiagnosticsPage = pathname === '/diagnostics';
  const isModelsPage = pathname === '/models';
  const isSettingsPage = pathname === '/settings' || pathname === '/about';

  let sectionLabel: string = t(($) => $.navigation.groups.workbench);
  let pageLabel: string = t(($) => $.navigation.live);
  if (isSessionPage) {
    sectionLabel = t(($) => $.navigation.groups.configuration);
    pageLabel = t(($) => $.navigation.sessionSettings);
  }
  if (isDiagnosticsPage) {
    sectionLabel = t(($) => $.navigation.groups.configuration);
    pageLabel = t(($) => $.navigation.diagnostics);
  }
  if (isModelsPage) {
    sectionLabel = t(($) => $.navigation.groups.configuration);
    pageLabel = t(($) => $.navigation.models);
  }
  if (isSettingsPage) {
    sectionLabel = t(($) => $.navigation.settings);
    pageLabel = t(($) => $.navigation.general);
    if (pathname === '/about') pageLabel = t(($) => $.navigation.about);
  }
  if (isRecordsPage) {
    sectionLabel = t(($) => $.navigation.groups.workbench);
    pageLabel = t(($) => $.navigation.live);
    if (isNewRecordPage) pageLabel = t(($) => $.records.editor.newTitle);
    if (isEditRecordPage) pageLabel = t(($) => $.records.editor.editTitle);
  }

  const statusState = snapshot?.status.state ?? 'idle';
  const legacySessionActive =
    statusState === 'running' || statusState === 'starting' || statusState === 'stopping';
  const activeRecords = records?.filter((record) => record.hasActiveSource) ?? [];
  const isSessionActive = legacySessionActive || activeRecords.length > 0;
  let liveDotClass = 'bg-muted-foreground/45';
  if (activeRecords.length > 0) liveDotClass = 'bg-success';
  let themeToggleLabel: string = t(($) => $.theme.switchToDark);
  let themeToggleIcon = moonIcon;
  if (theme === 'dark') {
    themeToggleLabel = t(($) => $.theme.switchToLight);
    themeToggleIcon = sunIcon;
  }

  useEffect(() => {
    let isDisposed = false;
    let unlisten: (() => void) | undefined;

    if (confirmBeforeQuit && isSessionActive && isTauri()) {
      void getCurrentWindow()
        .onCloseRequested((event) => {
          if (allowCloseRef.current) return;
          event.preventDefault();
          setIsQuitDialogOpen(true);
        })
        .then((dispose) => {
          if (isDisposed) dispose();
          else unlisten = dispose;
        });
    }

    return () => {
      isDisposed = true;
      unlisten?.();
    };
  }, [confirmBeforeQuit, isSessionActive]);

  function handleConfirmQuit() {
    allowCloseRef.current = true;
    setIsQuitDialogOpen(false);
    void getCurrentWindow()
      .close()
      .catch(() => {
        allowCloseRef.current = false;
      });
  }

  return (
    <div className="app-shell flex h-screen overflow-hidden bg-background text-[13px] text-foreground">
      <aside className="flex w-[212px] shrink-0 flex-col border-r border-border bg-background max-[1100px]:w-[188px]">
        <div
          data-tauri-drag-region
          className="flex h-[50px] shrink-0 items-center gap-2 pr-3 pl-[76px]"
        >
          <span className="grid size-6 shrink-0 place-items-center rounded-md bg-primary text-primary-foreground">
            <Icon icon={captionsIcon} className="size-3.5" />
          </span>
          <span className="truncate text-[13.5px] font-semibold tracking-tight">SonaSub</span>
        </div>

        <nav aria-label={t(($) => $.navigation.primary)} className="min-h-0 flex-1 px-2.5 pb-3">
          <p className="px-2.5 pt-1 pb-1 text-[11px] font-medium tracking-[0.04em] text-muted-foreground">
            {t(($) => $.navigation.groups.workbench)}
          </p>
          <RecordNav activeRecordId={activeRecordId} isLiveActive={isLivePage} />

          <p className="px-2.5 pt-4 pb-1 text-[11px] font-medium tracking-[0.04em] text-muted-foreground">
            {t(($) => $.navigation.groups.configuration)}
          </p>
          <Link
            to="/session"
            aria-current={currentPage(isSessionPage)}
            className={navigationItemClass(isSessionPage)}
          >
            <Icon icon={slidersHorizontalIcon} className="size-4 shrink-0" />
            <span className="truncate">{t(($) => $.navigation.sessionSettings)}</span>
          </Link>
          <Link
            to="/diagnostics"
            aria-current={currentPage(isDiagnosticsPage)}
            className={navigationItemClass(isDiagnosticsPage)}
          >
            <Icon icon={activityIcon} className="size-4 shrink-0" />
            <span className="truncate">{t(($) => $.navigation.diagnostics)}</span>
          </Link>
          <Link
            to="/models"
            aria-current={currentPage(isModelsPage)}
            className={navigationItemClass(isModelsPage)}
          >
            <Icon icon={packageIcon} className="size-4 shrink-0" />
            <span className="truncate">{t(($) => $.navigation.models)}</span>
          </Link>
        </nav>

        <div className="shrink-0 border-t border-border px-2.5 py-2">
          {isSettingsPage && (
            <div className="mb-1 ml-3 border-l border-border pl-2">
              <p className="px-2.5 pt-1 pb-0.5 text-[11px] font-medium text-muted-foreground">
                {t(($) => $.navigation.settings)}
              </p>
              <Link
                to="/settings"
                aria-current={currentPage(pathname === '/settings')}
                className={settingsSubItemClass(pathname === '/settings')}
              >
                {t(($) => $.navigation.general)}
              </Link>
              <Link
                to="/about"
                aria-current={currentPage(pathname === '/about')}
                className={settingsSubItemClass(pathname === '/about')}
              >
                {t(($) => $.navigation.about)}
              </Link>
            </div>
          )}
          <Link
            to="/settings"
            aria-current={currentPage(isSettingsPage)}
            className={navigationItemClass(isSettingsPage)}
          >
            <Icon icon={settingsIcon} className="size-4 shrink-0" />
            <span className="truncate">{t(($) => $.navigation.settings)}</span>
          </Link>
          <Link
            to="/"
            className="mt-1 flex h-[26px] items-center gap-2 rounded-md px-2.5 text-[11.5px] text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
          >
            <span className={`size-[7px] shrink-0 rounded-full ${liveDotClass}`} />
            <span className="truncate">
              {t(($) => $.records.nav.liveCount, { count: activeRecords.length })}
            </span>
          </Link>
        </div>
      </aside>

      <div className="flex min-w-0 flex-1 flex-col">
        <header
          data-tauri-drag-region
          className="flex h-[50px] shrink-0 items-center justify-between gap-4 border-b border-border px-4 md:px-5"
        >
          <div className="flex min-w-0 items-baseline gap-1.5 text-[13.5px] whitespace-nowrap">
            <span className="truncate text-muted-foreground max-[980px]:hidden">
              {sectionLabel}
            </span>
            <span className="text-muted-foreground/70 max-[980px]:hidden">/</span>
            <span className="truncate font-semibold text-foreground">{pageLabel}</span>
          </div>
          <button
            type="button"
            aria-label={themeToggleLabel}
            title={themeToggleLabel}
            className="grid size-7 shrink-0 place-items-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
            onClick={toggleTheme}
          >
            <Icon icon={themeToggleIcon} className="size-4" />
          </button>
        </header>

        <main className="flex min-h-0 flex-1 flex-col overflow-hidden">
          <Outlet />
        </main>
      </div>

      <AlertDialog open={isQuitDialogOpen} onOpenChange={(isOpen) => setIsQuitDialogOpen(isOpen)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t(($) => $.settings.general.quitConfirmTitle)}</AlertDialogTitle>
            <AlertDialogDescription>
              {t(($) => $.settings.general.quitConfirmPrompt)}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t(($) => $.settings.general.keepRunning)}</AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={handleConfirmQuit}>
              {t(($) => $.settings.general.quitAndStop)}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
