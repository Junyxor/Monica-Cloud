<script setup lang="ts">
import "@m3e/web/theme";
import "@m3e/web/button";
import "@m3e/web/icon";
import "@m3e/web/icon-button";
import "@m3e/web/progress-indicator";
import { onMounted } from "vue";
import ConfirmDialog from "./components/ConfirmDialog.vue";
import ToastHost from "./components/ToastHost.vue";
import { t } from "./i18n";
import { route } from "./lib/router";
import { initSession, sessionState } from "./lib/session";
import { activeScheme, themeColor, useThemePreferences } from "./lib/theme";
import AppShell from "./views/AppShell.vue";
import AuthView from "./views/AuthView.vue";
import ReceiveView from "./views/ReceiveView.vue";
import UnlockView from "./views/UnlockView.vue";

useThemePreferences();
onMounted(initSession);
</script>

<template>
  <m3e-theme :color="themeColor" :scheme="activeScheme" variant="expressive" motion="expressive" strong-focus class="app-theme">
    <ReceiveView v-if="route.name === 'receive'" :access-id="route.accessId" :send-key="route.key" />
    <div v-else-if="sessionState === 'loading'" class="splash">
      <img src="/monica-logo.png" alt="" />
      <m3e-circular-progress-indicator indeterminate></m3e-circular-progress-indicator>
    </div>
    <div v-else-if="sessionState === 'offline'" class="splash">
      <img src="/monica-logo.png" alt="" />
      <h1>{{ t("offline.title") }}</h1>
      <p>{{ t("offline.body") }}</p>
      <m3e-button variant="filled" @click="initSession">{{ t("common.retry") }}</m3e-button>
    </div>
    <AuthView v-else-if="sessionState === 'auth'" />
    <UnlockView v-else-if="sessionState === 'locked'" />
    <AppShell v-else />
    <ConfirmDialog />
    <ToastHost />
  </m3e-theme>
</template>
