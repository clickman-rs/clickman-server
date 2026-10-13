import { errorMessage } from '../graphql/errors';

// Keep drafts locked through both the write and its follow-up refresh.
export function createAction() {
  let busy = $state(false);
  let error = $state('');
  return {
    get busy() {
      return busy;
    },
    get error() {
      return error;
    },
    async run(operation: () => Promise<void>): Promise<void> {
      if (busy) return;
      busy = true;
      error = '';
      try {
        await operation();
      } catch (cause) {
        error = errorMessage(cause);
      } finally {
        busy = false;
      }
    },
  };
}
