<script setup lang="ts">
import { onMounted, ref } from "vue";
import { t } from "../i18n";
import type { DeviceRecord } from "../lib/client";
import { formatDateTime, formatRelative } from "../lib/format";
import { client, logout } from "../lib/session";
import { confirm, toastError } from "../lib/ui";

const devices = ref<DeviceRecord[]>([]);
const loading = ref(true);

const icons: Record<string, string> = { android: "smartphone", ios: "phone_iphone", web: "language", desktop: "computer", cli: "terminal" };

async function load() {
  loading.value = true;
  try {
    devices.value = await client.devices();
  } catch (e) {
    toastError(e);
  } finally {
    loading.value = false;
  }
}

async function revoke(device: DeviceRecord) {
  const ok = await confirm(t("devices.revokeTitle"), t(device.current ? "devices.revokeCurrent" : "devices.revokeBody", { name: device.name }), {
    danger: true,
    confirmLabel: t("devices.revoke")
  });
  if (!ok) return;
  try {
    if (device.current) return logout();
    await client.revokeDevice(device.id);
    await load();
  } catch (e) {
    toastError(e);
  }
}

onMounted(load);
</script>

<template>
  <div class="page">
    <header class="page-header">
      <div>
        <h1>{{ t("nav.devices") }}</h1>
        <p class="muted">{{ t("devices.subtitle") }}</p>
      </div>
    </header>
    <div class="card-list">
      <article v-for="device in devices" :key="device.id" class="send-card">
        <span class="send-icon"><m3e-icon :name="icons[device.type] ?? 'devices'"></m3e-icon></span>
        <div class="send-main">
          <div class="send-title">
            <strong>{{ device.name }}</strong>
            <span v-if="device.current" class="status-dot ok">{{ t("devices.thisDevice") }}</span>
          </div>
          <small class="muted">
            {{ t("devices.lastSeen", { time: formatRelative(device.lastSeenDate) }) }}
            <template v-if="device.lastIp"> · {{ device.lastIp }}</template>
            <template v-if="device.appVersion"> · {{ device.appVersion }}</template>
            · {{ t("devices.since", { time: formatDateTime(device.creationDate) }) }}
          </small>
        </div>
        <m3e-button variant="text" @click="revoke(device)">{{ t("devices.revoke") }}</m3e-button>
      </article>
      <p v-if="!loading && !devices.length" class="muted">{{ t("list.empty") }}</p>
    </div>
  </div>
</template>
