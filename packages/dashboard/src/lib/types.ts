import type { CachebayInstance } from 'cachebay';

export const clickmanSections = [
  'overview',
  'retention',
  'funnels',
  'segments',
  'settings',
] as const;
export type WebhookCreator = (input: {
  projectId: string;
  segmentId: string;
  name: string;
  kind: string;
  url: string;
  signingSecret: string;
}) => Promise<void>;
export type ClickmanSection = (typeof clickmanSections)[number];
export type DestinationCreator = (input: {
  projectId: string;
  name: string;
  mixpanelProjectId: string;
  username: string;
  secret: string;
  region: string;
}) => Promise<void>;

export interface ClickmanRoute {
  projectId: string | null;
  section: ClickmanSection;
}

export interface ClickmanWorkspaceProps {
  client: CachebayInstance;
  route: ClickmanRoute;
  onnavigate: (route: ClickmanRoute) => void | Promise<void>;
  createWebhook?: WebhookCreator;
  createDestination?: DestinationCreator;
  allowAccessKeys?: boolean;
  maxRetentionDays?: number;
}
