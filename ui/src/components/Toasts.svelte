<script lang="ts">
  import { app } from '../lib/store.svelte';
  import Icon from './Icon.svelte';
</script>

<div class="toasts" role="status" aria-live="polite">
  {#each app.toasts as toast (toast.id)}
    <button type="button" class="toast {toast.kind}" onclick={() => app.dismiss(toast.id)}>
      <Icon name={toast.kind === 'error' ? 'alert' : 'check'} />
      <span>{toast.text}</span>
    </button>
  {/each}
</div>
