import type { Interval, Scalar, StepDefinition } from '../client/types';

export interface FilterDraft {
  events: string[];
  alternatives: boolean;
  where: string;
}

export function filterDraft(filter?: StepDefinition): FilterDraft {
  return {
    events: filter ? (Array.isArray(filter.event) ? [...filter.event] : [filter.event]) : [''],
    alternatives: Array.isArray(filter?.event),
    where: filter?.where && Object.keys(filter.where).length ? JSON.stringify(filter.where) : '',
  };
}

function scalar(value: unknown): value is Scalar {
  return (
    value === null ||
    typeof value === 'string' ||
    typeof value === 'boolean' ||
    (typeof value === 'number' && Number.isFinite(value))
  );
}

function propertyFilters(text: string): StepDefinition['where'] {
  if (!text.trim()) return undefined;
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw new Error('Property filters must be valid JSON, for example {"plan":"paid"}.');
  }
  if (
    !value ||
    typeof value !== 'object' ||
    Array.isArray(value) ||
    Object.entries(value).some(
      ([key, item]) =>
        !key ||
        key.length > 128 ||
        !(scalar(item) || (Array.isArray(item) && item.length > 0 && item.every(scalar))),
    )
  )
    throw new Error(
      'Property filters must map property names to a value or a nonempty list of values.',
    );
  return value as StepDefinition['where'];
}

export function filterDefinition(draft: FilterDraft): StepDefinition {
  const events = [...draft.events];
  if (!events.length || events.some((event) => !event || Array.from(event).length > 200))
    throw new Error('Enter an event name for every alternative (up to 200 characters).');
  const where = propertyFilters(draft.where);
  return {
    event: events.length > 1 || draft.alternatives ? events : events[0],
    ...(where ? { where } : {}),
  };
}

export function duration(value: string, allowZero = false): Interval {
  if (allowZero && value === '0h') return value;
  const match = /^([1-9][0-9]*)([hd])$/.exec(value);
  const hours = match ? Number(match[1]) * (match[2] === 'd' ? 24 : 1) : 0;
  if (!Number.isSafeInteger(hours) || hours < 1 || hours > 365 * 24)
    throw new Error(
      'Use a duration between 1h and 365d.' + (allowZero ? ' Use 0h for no delay.' : ''),
    );
  return value as Interval;
}
