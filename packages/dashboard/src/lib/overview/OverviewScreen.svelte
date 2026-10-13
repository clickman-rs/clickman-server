<script lang="ts">
  import { Stat, ActivityChart } from '../ui/index';
  import { number, percentage, type ActiveDay, type EventTotal } from '../reports';
  let { days, events }: { days: ActiveDay[]; events: EventTotal[] } = $props();
  const today = $derived(days.at(-1));
  const totalEvents = $derived(events.reduce((sum, event) => sum + event.events, 0));
</script>

<div class="cm:mb-6 cm:grid cm:gap-4 cm:sm:grid-cols-2 cm:lg:grid-cols-4">
  <Stat
    label="Daily active users"
    value={number(today?.dau ?? 0)}
    detail="Today · identified users"
  /><Stat label="Weekly active users" value={number(today?.wau ?? 0)} detail="Last 7 days" /><Stat
    label="Monthly active users"
    value={number(today?.mau ?? 0)}
    detail="Last 30 days"
  /><Stat label="Events" value={number(totalEvents)} detail="Last 30 days · all events" />
</div>
<ActivityChart {days} />
<section class="cm-surface cm:mt-6 cm:p-6">
  <div class="cm:mb-4 cm:flex cm:justify-between">
    <h2>Top events</h2>
    <span class="cm:text-xs cm:text-muted">Last 30 days</span>
  </div>
  {#if events.length}<div class="cm:overflow-x-auto">
      <table>
        <thead
          ><tr
            ><th>Event</th><th class="cm:text-right">Events</th><th class="cm:text-right">Users</th
            ><th class="cm:pl-8">Share</th></tr
          ></thead
        ><tbody
          >{#each events as event}<tr
              ><td class="cm:font-medium">{event.event}</td><td
                class="cm:text-right cm:tabular-nums">{number(event.events)}</td
              ><td class="cm:text-right cm:tabular-nums">{number(event.actors)}</td><td
                class="cm:w-1/4 cm:pl-8"
                ><div class="cm:flex cm:items-center cm:gap-3">
                  <div class="cm:h-1.5 cm:flex-1 cm:rounded cm:bg-brand-light">
                    <div
                      class="cm:h-1.5 cm:rounded cm:bg-brand"
                      style:width={`${Math.max(1, (event.events / Math.max(1, totalEvents)) * 100)}%`}
                    ></div>
                  </div>
                  <span class="cm:w-10 cm:text-right cm:text-xs cm:text-muted"
                    >{percentage(event.events, totalEvents)}</span
                  >
                </div></td
              ></tr
            >{/each}</tbody
        >
      </table>
    </div>{:else}<div class="cm-empty">
      No events yet. Create a source key in Settings and connect your application.
    </div>{/if}
</section>
