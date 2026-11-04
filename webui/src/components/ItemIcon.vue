<script setup lang="ts">
import { computed } from "vue";
import { typeInfo } from "../lib/items";

const props = defineProps<{ type: number; name?: string; size?: "small" | "large" }>();
const info = computed(() => typeInfo(props.type));
// Stable hue per item name so the list is easy to scan.
const hue = computed(() => {
  let hash = 0;
  for (const char of props.name ?? info.value.key) hash = (hash * 31 + char.charCodeAt(0)) | 0;
  return Math.abs(hash) % 360;
});
const letter = computed(() => (props.name ?? "").trim().slice(0, 1).toUpperCase());
</script>

<template>
  <span class="item-icon" :class="[size ?? 'small', `type-${info.key}`]" :style="{ '--hue': hue }">
    <span v-if="info.key === 'login' && letter" class="letter">{{ letter }}</span>
    <m3e-icon v-else :name="info.icon"></m3e-icon>
  </span>
</template>
