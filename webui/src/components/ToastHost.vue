<script setup lang="ts">
import { toasts } from "../lib/ui";
</script>

<template>
  <div class="toast-host" role="status" aria-live="polite">
    <TransitionGroup name="toast">
      <div v-for="item in toasts" :key="item.id" class="toast" :class="item.tone">
        <m3e-icon :name="item.tone === 'error' ? 'error' : 'check_circle'"></m3e-icon>
        <span>{{ item.message }}</span>
      </div>
    </TransitionGroup>
  </div>
</template>
