<script setup lang="ts">
import { ref } from "vue";
import { t } from "../i18n";

defineProps<{ autocomplete?: string; placeholder?: string; mono?: boolean; autofocus?: boolean }>();
const model = defineModel<string>({ default: "" });
const visible = ref(false);
</script>

<template>
  <div class="input-with-action">
    <input
      v-model="model"
      :type="visible ? 'text' : 'password'"
      :autocomplete="autocomplete ?? 'off'"
      :placeholder="placeholder"
      :class="{ mono }"
      :autofocus="autofocus"
      spellcheck="false"
    />
    <m3e-icon-button type="button" :aria-label="t(visible ? 'common.hide' : 'common.show')" @click="visible = !visible">
      <m3e-icon :name="visible ? 'visibility_off' : 'visibility'"></m3e-icon>
    </m3e-icon-button>
  </div>
</template>
