<script lang="ts">
  import { number, percentage, type Cohort } from '../reports';
  let { cohorts }: { cohorts: Cohort[] } = $props();
</script>

<section class="cm-surface cm:p-6">
  <h2>Cohort retention</h2>
  <p class="cm:mb-5 cm:mt-1 cm:text-sm cm:text-muted">
    Users who return exactly 1, 7 and 30 days after their first activity.
  </p>
  {#if cohorts.length}<div class="cm:overflow-x-auto">
      <table>
        <thead
          ><tr
            ><th>First active day</th><th class="cm:text-right">Users</th><th class="cm:text-center"
              >Day 1</th
            ><th class="cm:text-center">Day 7</th><th class="cm:text-center">Day 30</th></tr
          ></thead
        ><tbody
          >{#each [...cohorts].reverse() as cohort}<tr
              ><td>{cohort.day}</td><td class="cm:text-right cm:tabular-nums"
                >{number(cohort.size)}</td
              >{#each [cohort.d1, cohort.d7, cohort.d30] as count}<td class="cm:text-center"
                  ><span
                    class="cm:inline-block cm:min-w-16 cm:rounded cm:px-3 cm:py-2 cm:font-medium"
                    style:background={count === null
                      ? '#f5f8f6'
                      : `rgba(20,110,89,${0.05 + (cohort.size ? count / cohort.size : 0) * 0.2})`}
                    >{percentage(count, cohort.size)}</span
                  ></td
                >{/each}</tr
            >{/each}</tbody
        >
      </table>
    </div>{:else}<div class="cm-empty">
      Retention appears after identified users start sending events.
    </div>{/if}
</section>
