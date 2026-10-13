export type Scalar = string | number | boolean | null;
export type Interval = `${number}h` | `${number}d`;
export interface StepDefinition {
  event: string | string[];
  where?: Record<string, Scalar | Scalar[]>;
}
export interface FunnelDefinition {
  key: string;
  name: string;
  steps: [StepDefinition, StepDefinition, ...StepDefinition[]];
  window?: Interval;
  range?: Interval;
  breakdown?: string | null;
}
export interface SegmentDefinition {
  key: string;
  name: string;
  rule:
    | { kind: 'event'; filter: StepDefinition; range: Interval }
    | {
        kind: 'funnel';
        funnel_key: string;
        at_step: number;
        exclude_later?: boolean;
        wait?: Interval;
      }
    | {
        kind: 'inactive_after';
        entry: StepDefinition;
        absent: StepDefinition;
        after: Interval;
        range: Interval;
      };
}
export interface WebhookConfiguration {
  url?: string | null;
  endpoint_ref?: string | null;
  signing_ref: string;
}
