<script setup lang="ts">
import { onMounted, ref } from "vue";
import PasswordInput from "../components/PasswordInput.vue";
import { t } from "../i18n";
import { ApiError, MonicaClient, type PublicSend } from "../lib/client";
import * as mc from "../lib/crypto";
import { formatBytes, formatDateTime } from "../lib/format";
import { copy, errorMessage } from "../lib/ui";
import { saveFile } from "../lib/vault";

const props = defineProps<{ accessId: string; sendKey: string }>();
const api = new MonicaClient("");
const state = ref<"loading" | "password" | "ready" | "gone" | "error">("loading");
const error = ref("");
const password = ref("");
const send = ref<PublicSend | null>(null);
const name = ref("");
const text = ref("");
const fileName = ref("");
const downloading = ref(false);
let keys: mc.SendKeys;

async function load(passwordHash?: string) {
  state.value = "loading";
  error.value = "";
  try {
    keys = await mc.deriveSendKeys(mc.b64urlDecode(props.sendKey));
    const result = await api.accessSend(props.accessId, passwordHash);
    name.value = await mc.decryptString(keys.contentKey, result.name);
    if (result.text) text.value = await mc.decryptString(keys.contentKey, result.text);
    if (result.fileName) fileName.value = await mc.decryptString(keys.contentKey, result.fileName);
    send.value = result;
    state.value = "ready";
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) {
      state.value = "password";
      if (passwordHash) error.value = t("receive.wrongPassword");
    } else if (e instanceof ApiError && e.status === 404) {
      state.value = "gone";
    } else {
      state.value = "error";
      error.value = e instanceof DOMException || e instanceof TypeError ? t("receive.badLink") : errorMessage(e);
    }
  }
}

async function submitPassword() {
  if (!password.value) return;
  await load(await mc.sendPasswordHash(password.value, keys.sendKey));
}

async function download() {
  if (!send.value?.downloadToken) return;
  downloading.value = true;
  try {
    const blob = await api.downloadSendFile(props.accessId, send.value.downloadToken);
    saveFile(fileName.value || "download", await mc.decryptBlob(keys.contentKey, blob));
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    downloading.value = false;
  }
}

onMounted(() => load());
</script>

<template>
  <div class="auth-page single receive">
    <section class="auth-panel">
      <div class="auth-card">
        <div class="auth-mobile-brand always"><img src="/monica-logo.png" alt="" /><span>Monica Send</span></div>
        <div v-if="state === 'loading'" class="center"><m3e-circular-progress-indicator indeterminate></m3e-circular-progress-indicator></div>

        <form v-else-if="state === 'password'" class="form-group" @submit.prevent="submitPassword">
          <h2>{{ t("receive.protected") }}</h2>
          <p class="muted">{{ t("receive.protectedBody") }}</p>
          <label class="field"><span>{{ t("send.password") }}</span><PasswordInput v-model="password" autofocus /></label>
          <p v-if="error" class="form-error">{{ error }}</p>
          <m3e-button variant="filled" type="submit" class="wide">{{ t("receive.open") }}</m3e-button>
          <button type="submit" hidden tabindex="-1"></button>
        </form>

        <template v-else-if="state === 'ready' && send">
          <div>
            <h2>{{ name }}</h2>
            <p class="muted">
              {{ send.creatorIdentifier ? t("receive.from", { email: send.creatorIdentifier }) : t("receive.anonymous") }}
              <template v-if="send.expirationDate"> · {{ t("receive.expires", { time: formatDateTime(send.expirationDate) }) }}</template>
            </p>
          </div>
          <template v-if="send.type === 0">
            <pre class="received-text">{{ text }}</pre>
            <m3e-button variant="filled" class="wide" @click="copy(text)"><m3e-icon slot="icon" name="content_copy"></m3e-icon>{{ t("common.copy") }}</m3e-button>
          </template>
          <template v-else>
            <div class="attachment-row big">
              <m3e-icon name="description"></m3e-icon>
              <div><strong>{{ fileName }}</strong><small>{{ formatBytes(send.size) }}</small></div>
            </div>
            <p v-if="error" class="form-error">{{ error }}</p>
            <m3e-button variant="filled" class="wide" :disabled="downloading" @click="download">
              <m3e-icon slot="icon" name="download"></m3e-icon>{{ downloading ? t("receive.decrypting") : t("common.download") }}
            </m3e-button>
          </template>
          <p class="notice"><m3e-icon name="lock"></m3e-icon>{{ t("receive.e2e") }}</p>
        </template>

        <div v-else class="center">
          <span class="empty-icon"><m3e-icon :name="state === 'gone' ? 'link_off' : 'error'"></m3e-icon></span>
          <h2>{{ state === "gone" ? t("receive.goneTitle") : t("receive.errorTitle") }}</h2>
          <p class="muted">{{ state === "gone" ? t("receive.goneBody") : error }}</p>
        </div>
      </div>
    </section>
  </div>
</template>
