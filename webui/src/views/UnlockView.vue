<script setup lang="ts">
import { ref } from "vue";
import PasswordInput from "../components/PasswordInput.vue";
import { t } from "../i18n";
import { account, logout, unlock } from "../lib/session";
import { errorMessage } from "../lib/ui";

const password = ref("");
const busy = ref(false);
const error = ref("");

async function submit() {
  if (!password.value) return;
  busy.value = true;
  error.value = "";
  try {
    await unlock(password.value);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
    password.value = "";
  }
}
</script>

<template>
  <div class="auth-page single">
    <section class="auth-panel">
      <form class="auth-card" @submit.prevent="submit">
        <div class="lock-badge"><m3e-icon name="lock"></m3e-icon></div>
        <div class="center">
          <h2>{{ t("unlock.title") }}</h2>
          <p class="muted">{{ t("unlock.subtitle", { email: account?.email ?? "" }) }}</p>
        </div>
        <label class="field">
          <span>{{ t("auth.masterPassword") }}</span>
          <PasswordInput v-model="password" autocomplete="current-password" autofocus />
        </label>
        <p v-if="error" class="form-error" role="alert">{{ error }}</p>
        <m3e-button variant="filled" class="wide" type="submit" :disabled="busy">
          <m3e-circular-progress-indicator v-if="busy" slot="icon" indeterminate diameter="18"></m3e-circular-progress-indicator>
          {{ t("auth.unlockVault") }}
        </m3e-button>
        <button type="submit" hidden tabindex="-1"></button>
        <m3e-button variant="text" class="wide" type="button" @click="logout">{{ t("unlock.switchAccount") }}</m3e-button>
      </form>
    </section>
  </div>
</template>
