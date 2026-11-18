import { createRootRoute, createRoute } from '@tanstack/react-router';

import { AppLayout } from './components/app-layout';
import { DiagnosticsPage } from './routes/diagnostics';
import { ModelsPage } from './routes/models';
import { OverviewPage } from './routes/overview';
import { EditRecordPage, NewRecordPage } from './routes/record-editor';
import { SessionSettingsPage } from './routes/session-settings';
import { AboutSettingsPage, GeneralSettingsPage } from './routes/settings';

const rootRoute = createRootRoute({ component: AppLayout });

const overviewRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  component: OverviewPage,
  validateSearch: (search: Record<string, unknown>): { record?: string } => {
    let record: string | undefined;
    if (typeof search.record === 'string' && search.record.length > 0) {
      record = search.record;
    }
    if (!record) return {};
    return { record };
  },
});

const newRecordRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/records/new',
  component: NewRecordPage,
});

const editRecordRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/records/$recordId/edit',
  component: EditRecordPage,
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

const modelsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/models',
  component: ModelsPage,
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
  newRecordRoute,
  editRecordRoute,
  sessionSettingsRoute,
  diagnosticsRoute,
  modelsRoute,
  settingsRoute,
  aboutRoute,
]);
