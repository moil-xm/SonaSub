import { createRootRoute, createRoute } from '@tanstack/react-router';

import { AppLayout } from './components/app-layout';
import { DiagnosticsPage } from './routes/diagnostics';
import { OverviewPage } from './routes/overview';
import { SessionSettingsPage } from './routes/session-settings';
import { AboutSettingsPage, GeneralSettingsPage } from './routes/settings';

const rootRoute = createRootRoute({ component: AppLayout });

const overviewRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  component: OverviewPage,
});

const sessionSettingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/session',
  component: SessionSettingsPage,
});

const diagnosticsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/diagnostics',
  component: DiagnosticsPage,
});

const settingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/settings',
  component: GeneralSettingsPage,
});

const aboutRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/about',
  component: AboutSettingsPage,
});

export const routeTree = rootRoute.addChildren([
  overviewRoute,
  sessionSettingsRoute,
  diagnosticsRoute,
  settingsRoute,
  aboutRoute,
]);
