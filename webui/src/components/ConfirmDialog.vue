<script setup lang="ts">
import "@m3e/web/dialog";
import { computed } from "vue";
import { t } from "../i18n";
import { confirmState } from "../lib/ui";

const request = computed(() => confirmState.request);

function answer(ok: boolean) {
  const current = confirmState.request;
  confirmState.request = null;
  current?.resolve(ok);
}
</script>

<template>
  <m3e-dialog :open="Boolean(request)" @closed="request && answer(false)">
    <span slot="header">{{ request?.title }}</span>
    <p class="dialog-text">{{ request?.message }}</p>
    <div slot="actions" end>
      <m3e-button variant="text" @click="answer(false)">{{ t("common.cancel") }}</m3e-button>
      <m3e-button :variant="request?.danger ? 'filled' : 'tonal'" :class="{ danger: request?.danger }" @click="answer(true)">
        {{ request?.confirmLabel }}
      </m3e-button>
    </div>
  </m3e-dialog>
</template>
