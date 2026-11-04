<script setup lang="ts">
import { languageLabel, locale, locales, setLocale, t, type Locale, type MessageKey } from "../i18n";
import { paletteId, palettes, schemePreference, setPalette, setScheme, type SchemePreference } from "../lib/theme";

const schemes: { id: SchemePreference; icon: string }[] = [
  { id: "auto", icon: "brightness_auto" },
  { id: "light", icon: "light_mode" },
  { id: "dark", icon: "dark_mode" }
];
</script>

<template>
  <section class="settings-card">
    <h2>{{ t("settings.appearance") }}</h2>
    <label class="field">
      <span>{{ t("prefs.language") }}</span>
      <select :value="locale" @change="setLocale(($event.target as HTMLSelectElement).value as Locale)">
        <option v-for="item in locales" :key="item" :value="item">{{ languageLabel[item] }}</option>
      </select>
    </label>
    <div class="field">
      <span>{{ t("prefs.scheme") }}</span>
      <div class="segmented">
        <button v-for="item in schemes" :key="item.id" type="button" :class="{ selected: schemePreference === item.id }" @click="setScheme(item.id)">
          <m3e-icon :name="item.icon"></m3e-icon>{{ t(`scheme.${item.id}` as MessageKey) }}
        </button>
      </div>
    </div>
    <div class="field">
      <span>{{ t("prefs.palette") }}</span>
      <div class="palette-grid">
        <button
          v-for="item in palettes"
          :key="item.id"
          type="button"
          class="palette-button"
          :class="{ selected: item.id === paletteId }"
          :aria-label="t(`palettes.${item.id}` as MessageKey)"
          :title="t(`palettes.${item.id}` as MessageKey)"
          @click="setPalette(item.id)"
        >
          <span class="swatch" :style="{ '--swatch': item.color, '--accent': item.accent }"></span>
        </button>
      </div>
    </div>
  </section>
</template>
