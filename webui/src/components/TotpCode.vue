<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { t } from "../i18n";
import { parseTotp, secondsRemaining, totpCode } from "../lib/totp";
import { copy } from "../lib/ui";

const props = defineProps<{ secret: string; digits?: number; period?: number; algorithm?: string }>();
const params = computed(() => parseTotp(props.secret, { digits: props.digits, period: props.period, algorithm: props.algorithm }));
const code = ref("");
const remaining = ref(30);
let timer: number | undefined;

async function refresh() {
  if (!params.value) return;
  remaining.value = secondsRemaining(params.value.period);
  code.value = await totpCode(params.value);
}

const pretty = computed(() => (code.value.length === 6 ? `${code.value.slice(0, 3)} ${code.value.slice(3)}` : code.value));
const progress = computed(() => (params.value ? remaining.value / params.value.period : 0));

watch(params, refresh);
onMounted(() => {
  refresh();
  timer = window.setInterval(refresh, 1000);
});
onUnmounted(() => window.clearInterval(timer));
</script>

<template>
  <div v-if="params" class="totp">
    <svg class="totp-ring" viewBox="0 0 36 36" :class="{ urgent: remaining <= 5 }" aria-hidden="true">
      <circle cx="18" cy="18" r="15" class="track" />
      <circle cx="18" cy="18" r="15" class="bar" :style="{ strokeDashoffset: `${94.25 * (1 - progress)}` }" />
      <text x="18" y="22" text-anchor="middle">{{ remaining }}</text>
    </svg>
    <span class="totp-code mono">{{ pretty }}</span>
    <m3e-icon-button :aria-label="t('common.copy')" @click="copy(code)"><m3e-icon name="content_copy"></m3e-icon></m3e-icon-button>
  </div>
  <span v-else class="muted">{{ t("item.totpInvalid") }}</span>
</template>
