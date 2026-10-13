export interface ActiveDay {
  day: string;
  dau: number;
  wau: number;
  mau: number;
}
export interface Cohort {
  day: string;
  size: number;
  d1: number | null;
  d7: number | null;
  d30: number | null;
}
export interface EventTotal {
  event: string;
  events: number;
  actors: number;
}
export interface FunnelReport {
  key: string;
  name: string;
  steps: { events: string[]; actors: number; median_seconds: number | null }[];
  breakdown_key: string | null;
  breakdown: { value: unknown; actors: number[] }[];
}
export interface Settings {
  filter_fragments: string[];
  max_event_age_days: number;
  dedup_window_days: number;
  retention_days: number | null;
  rotation_lag_seconds: number;
  rate_limit: { per_second: number; burst: number };
}
export const number = (n: number) => n.toLocaleString();
export const percentage = (count: number | null, total: number) =>
  count === null ? '—' : `${total ? Math.round((count / total) * 100) : 0}%`;
