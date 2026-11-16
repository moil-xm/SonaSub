import { createRootRoute, createRoute } from '@tanstack/react-router';

import { AppLayout } from './components/app-layout';
import { AboutPage } from './routes/about';
import { OverviewPage } from './routes/overview';

const rootRoute = createRootRoute({ component: AppLayout });

const overviewRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  component: OverviewPage,
});

const aboutRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/about',
  component: AboutPage,
});

export const routeTree = rootRoute.addChildren([overviewRoute, aboutRoute]);
