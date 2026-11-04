<script setup lang="ts">
import { computed, ref } from "vue";
import PasswordInput from "../components/PasswordInput.vue";
import { t } from "../i18n";
import { lastEmail, login, register, serverConfig } from "../lib/session";
import { errorMessage } from "../lib/ui";

const firstRun = computed(() => serverConfig.value?.initialized === false);
const mode = ref<"login" | "register">(firstRun.value ? "register" : "login");
const email = ref(lastEmail.value);
const name = ref("");
const password = ref("");
const confirmPassword = ref("");
const busy = ref(false);
const error = ref("");

const strength = computed(() => {
  const value = password.value;
  let score = 0;
  if (value.length >= 12) score++;
  if (value.length >= 16) score++;
  if (/[a-z]/.test(value) && /[A-Z]/.test(value)) score++;
  if (/\d/.test(value)) score++;
  if (/[^A-Za-z0-9]/.test(value)) score++;
  return Math.min(4, score);
});

async function submit() {
  error.value = "";
  if (!email.value.trim() || !password.value) {
    error.value = t("auth.errorRequired");
    return;
  }
  if (mode.value === "register") {
    if (password.value.length < 12) {
      error.value = t("auth.errorShort");
      return;
    }
    if (password.value !== confirmPassword.value) {
      error.value = t("auth.errorMismatch");
      return;
    }
  }
  busy.value = true;
  try {
    if (mode.value === "login") await login(email.value, password.value);
    else await register(email.value, name.value, password.value);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
    password.value = mode.value === "login" ? "" : password.value;
  }
}

function switchMode(next: "login" | "register") {
  mode.value = next;
  error.value = "";
}
</script>

<template>
  <div class="auth-page">
    <section class="auth-hero">
      <div class="brand"><img src="/monica-logo.png" alt="" /><span>Monica Cloud</span></div>
      <div class="hero-copy">
        <h1>{{ t("auth.heroTitle") }}</h1>
        <p>{{ t("auth.heroBody") }}</p>
      </div>
      <ul class="hero-points">
        <li><span class="hero-icon"><m3e-icon name="lock"></m3e-icon></span>{{ t("auth.pointZeroKnowledge") }}</li>
        <li><span class="hero-icon"><m3e-icon name="devices"></m3e-icon></span>{{ t("auth.pointSync") }}</li>
        <li><span class="hero-icon"><m3e-icon name="dns"></m3e-icon></span>{{ t("auth.pointSelfHosted") }}</li>
      </ul>
      <small v-if="serverConfig" class="hero-version">{{ serverConfig.name }} {{ serverConfig.version }}</small>
    </section>

    <section class="auth-panel">
      <form class="auth-card" @submit.prevent="submit">
        <div class="auth-mobile-brand"><img src="/monica-logo.png" alt="" /><span>Monica Cloud</span></div>
        <div>
          <h2>{{ mode === "login" ? t("auth.loginTitle") : firstRun ? t("auth.ownerTitle") : t("auth.registerTitle") }}</h2>
          <p class="muted">{{ mode === "login" ? t("auth.loginSubtitle") : firstRun ? t("auth.ownerSubtitle") : t("auth.registerSubtitle") }}</p>
        </div>

        <label class="field">
          <span>{{ t("auth.email") }}</span>
          <input v-model="email" type="email" autocomplete="username" required />
        </label>
        <label v-if="mode === 'register'" class="field">
          <span>{{ t("auth.name") }}</span>
          <input v-model="name" autocomplete="name" />
        </label>
        <label class="field">
          <span>{{ t("auth.masterPassword") }}</span>
          <PasswordInput v-model="password" :autocomplete="mode === 'login' ? 'current-password' : 'new-password'" />
        </label>
        <template v-if="mode === 'register'">
          <div class="strength" :data-score="strength">
            <span v-for="i in 4" :key="i" :class="{ on: i <= strength }"></span>
            <small>{{ t(`strength.${strength}` as never) }}</small>
          </div>
          <label class="field">
            <span>{{ t("auth.confirmPassword") }}</span>
            <PasswordInput v-model="confirmPassword" autocomplete="new-password" />
          </label>
          <p class="notice"><m3e-icon name="info"></m3e-icon>{{ t("auth.noRecovery") }}</p>
        </template>

        <p v-if="error" class="form-error" role="alert">{{ error }}</p>

        <m3e-button variant="filled" class="wide" type="submit" :disabled="busy">
          <m3e-circular-progress-indicator v-if="busy" slot="icon" indeterminate diameter="18"></m3e-circular-progress-indicator>
          {{ busy ? t("auth.deriving") : mode === "login" ? t("auth.unlockVault") : t("auth.createAccount") }}
        </m3e-button>

        <button type="submit" hidden tabindex="-1"></button>
        <p v-if="mode === 'login' && serverConfig?.signupsAllowed" class="switch-line">
          {{ t("auth.noAccount") }} <a href="#" @click.prevent="switchMode('register')">{{ t("auth.createAccount") }}</a>
        </p>
        <p v-if="mode === 'register' && !firstRun" class="switch-line">
          {{ t("auth.haveAccount") }} <a href="#" @click.prevent="switchMode('login')">{{ t("auth.loginTitle") }}</a>
        </p>
      </form>
    </section>
  </div>
</template>
