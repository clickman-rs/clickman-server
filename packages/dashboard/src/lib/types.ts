import type { CachebayInstance } from 'cachebay';

export const clickmanSections = ['overview', 'retention', 'funnels', 'settings'] as const;
export type ClickmanSection = (typeof clickmanSections)[number];

export interface ClickmanRoute {
  projectId: string | null;
  section: ClickmanSection;
}

export interface ClickmanWorkspaceProps {
  client: CachebayInstance;
  route: ClickmanRoute;
  onnavigate: (route: ClickmanRoute) => void | Promise<void>;
}
