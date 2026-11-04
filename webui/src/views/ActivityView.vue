<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { t, type MessageKey } from "../i18n";
import type { EventRecord } from "../lib/client";
import { formatDateTime } from "../lib/format";
import { client } from "../lib/session";
import { toastError } from "../lib/ui";

const props = defineProps<{ admin: boolean }>();
const events = ref<EventRecord[]>([]);
const loading = ref(false);
const done = ref(false);
const PAGE = 100;

const icons: Record<string, string> = {
  auth: "login",
  account: "person",
  item: "key",
  items: "inventory_2",
  folder: "folder",
  attachment: "attach_file",
  send: "send",
  device: "devices",
  admin: "admin_panel_settings"
};

function label(action: string) {
  const key = `event.${action}` as MessageKey;
  const text = t(key);
  return text === key ? action : text;
}

async function load(more = false) {
  loading.value = true;
  try {
    const before = more ? events.value[events.value.length - 1]?.date : undefined;
    const page = props.admin ? await client.adminEvents(PAGE, before) : await client.events(PAGE, before);
    events.value = more ? [...events.value, ...page] : page;
    done.value = page.length < PAGE;
  } catch (e) {
    toastError(e);
  } finally {
    loading.value = false;
  }
}

watch(() => props.admin, () => load());
onMounted(() => load());
</script>

<template>
  <div class="page">
    <header class="page-header">
      <div>
        <h1>{{ admin ? t("nav.adminActivity") : t("nav.activity") }}</h1>
        <p class="muted">{{ admin ? t("activity.adminSubtitle") : t("activity.subtitle") }}</p>
      </div>
      <m3e-icon-button :aria-label="t('common.refresh')" @click="load()"><m3e-icon name="refresh"></m3e-icon></m3e-icon-button>
    </header>
    <ol class="timeline">
      <li v-for="event in events" :key="event.id">
        <span class="timeline-icon"><m3e-icon :name="icons[event.action.split('.')[0]] ?? 'bolt'"></m3e-icon></span>
        <div>
          <strong>{{ label(event.action) }}</strong>
          <small class="muted">
            <template v-if="admin">{{ event.actor }} · </template>
            {{ formatDateTime(event.date) }}
            <template v-if="event.deviceName"> · {{ event.deviceName }}</template>
            <template v-if="event.ip"> · {{ event.ip }}</template>
          </small>
        </div>
        <code v-if="event.target" class="timeline-target" :title="event.target">{{ event.target }}</code>
      </li>
    </ol>
    <p v-if="!loading && !events.length" class="muted">{{ t("list.empty") }}</p>
    <m3e-button v-if="events.length && !done" variant="text" :disabled="loading" @click="load(true)">{{ t("common.loadMore") }}</m3e-button>
  </div>
</template>
