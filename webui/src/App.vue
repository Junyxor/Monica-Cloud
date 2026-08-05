<script setup lang="ts">
import "@m3e/web/theme";
import "@m3e/web/app-bar";
import "@m3e/web/badge";
import "@m3e/web/button";
import "@m3e/web/card";
import "@m3e/web/chips";
import "@m3e/web/icon";
import "@m3e/web/icon-button";
import "@m3e/web/progress-indicator";
import "@m3e/web/switch";
import "@m3e/web/toolbar";
import { computed, onMounted, ref } from "vue";
import AppearancePanel from "./components/AppearancePanel.vue";
import DataTable from "./components/DataTable.vue";
import ProjectDetail from "./components/ProjectDetail.vue";
import { t } from "./i18n";
import type { MonicaServerSnapshot } from "./lib/api";
import { loadServerSnapshot } from "./lib/api";
import { defaultAction, featureModules, moduleGroups, modulesById, projectSectionIds, stateClass, stateLabel, type SectionId } from "./lib/modules";
import { activeScheme, themeColor, useThemePreferences } from "./lib/theme";

const snapshot = ref<MonicaServerSnapshot | null>(null);
const authenticated = ref(false);
const activeSection = ref<SectionId>("dashboard");
const query = ref("");
const mobileNavOpen = ref(false);
const email = ref("owner@monica.local");
const password = ref("");
const loginError = ref("");

useThemePreferences();

const groupedModules = computed(() => {
  return moduleGroups.map((group) => ({
    ...group,
    items: featureModules.filter((module) => module.group === group.id)
  }));
});

const activeModule = computed(() => modulesById.get(activeSection.value) ?? featureModules[0]);
const action = computed(() => defaultAction(activeModule.value));
const moduleContext = computed(() => {
  const data = snapshot.value;
  if (!data) return null;
  return {
    snapshot: data,
    query: query.value.trim().toLowerCase(),
    entriesForProject,
    attachmentsForProject,
    projectTitle
  };
});
const rows = computed(() => (moduleContext.value ? activeModule.value.rows(moduleContext.value) : []));
const selectedProject = computed(() => (activeModule.value.projectDetail ? currentProjects.value[0] : undefined));
const selectedEntries = computed(() => (selectedProject.value ? entriesForProject(selectedProject.value.id) : []));
const selectedAttachments = computed(() => (selectedProject.value ? attachmentsForProject(selectedProject.value.id) : []));
const currentProjects = computed(() => {
  const ids = new Set(rows.value.map((row) => row.id));
  return snapshot.value?.projects.filter((project) => ids.has(project.id)) ?? [];
});
const showDetail = computed(() => Boolean(selectedProject.value && projectSectionIds.includes(activeModule.value.id)));

onMounted(async () => {
  snapshot.value = await loadServerSnapshot();
});

function navigate(section: SectionId) {
  activeSection.value = section;
  mobileNavOpen.value = false;
}

function login() {
  if (!email.value.trim() || !password.value.trim()) {
    loginError.value = t("auth.error");
    return;
  }
  loginError.value = "";
  authenticated.value = true;
}

function entriesForProject(projectId: string) {
  return snapshot.value?.entries.filter((entry) => entry.projectId === projectId) ?? [];
}

function attachmentsForProject(projectId: string) {
  return snapshot.value?.attachments.filter((attachment) => attachment.projectId === projectId) ?? [];
}

function projectTitle(projectId: string) {
  return snapshot.value?.projects.find((project) => project.id === projectId)?.title ?? projectId;
}
</script>

<template>
  <m3e-theme :color="themeColor" :scheme="activeScheme" variant="expressive" motion="expressive" strong-focus>
    <div v-if="!snapshot" class="loading">
      <img src="/monica-logo.png" alt="" />
      <h1>{{ t("app.name") }}</h1>
      <m3e-circular-progress-indicator indeterminate></m3e-circular-progress-indicator>
    </div>

    <form v-else-if="!authenticated" class="login" @submit.prevent="login">
      <div class="brand">
        <img src="/monica-logo.png" alt="" />
        <span>{{ t("app.name") }}</span>
      </div>
      <m3e-card variant="outlined" class="login-card">
        <div slot="content" class="stack">
          <div class="avatar-icon"><m3e-icon name="lock"></m3e-icon></div>
          <div>
            <h1>{{ t("auth.login") }}</h1>
            <p>{{ t("auth.subtitle") }}</p>
          </div>
          <label class="field">
            <span>{{ t("auth.email") }}</span>
            <input v-model="email" autocomplete="username" type="email" />
          </label>
          <label class="field">
            <span>{{ t("auth.password") }}</span>
            <input v-model="password" autocomplete="current-password" type="password" />
          </label>
          <p v-if="loginError" class="form-error">{{ loginError }}</p>
          <m3e-button variant="filled" type="submit">
            <m3e-icon slot="icon" name="verified_user"></m3e-icon>
            {{ t("auth.login") }}
          </m3e-button>
          <m3e-chip><m3e-icon slot="icon" name="key"></m3e-icon>{{ t("auth.encrypted") }}</m3e-chip>
        </div>
      </m3e-card>
    </form>

    <div v-else class="shell" :class="{ 'nav-open': mobileNavOpen }">
      <aside class="sidebar">
        <div class="brand sidebar-brand">
          <img src="/monica-logo.png" alt="" />
          <span>{{ t("app.shortName") }}</span>
        </div>
        <nav>
          <section v-for="group in groupedModules" :key="group.id">
            <p class="nav-title">{{ t(group.labelKey) }}</p>
            <button
              v-for="module in group.items"
              :key="module.id"
              class="nav-item"
              :data-section="module.id"
              :class="{ selected: activeSection === module.id }"
              type="button"
              @click="navigate(module.id)"
            >
              <m3e-icon :name="module.icon"></m3e-icon>
              <span>{{ t(module.navKey) }}</span>
              <m3e-badge v-if="module.badge?.(snapshot)" size="medium">{{ module.badge(snapshot) }}</m3e-badge>
            </button>
          </section>
        </nav>
        <div class="sidebar-footer">
          <m3e-chip><m3e-icon slot="icon" name="verified_user"></m3e-icon>{{ t("app.zeroKnowledge") }}</m3e-chip>
          <m3e-button variant="filled"><m3e-icon slot="icon" name="backup"></m3e-icon>{{ t("actions.newBackup") }}</m3e-button>
        </div>
      </aside>

      <main>
        <Transition name="page-fade" mode="out-in">
          <div :key="activeModule.id">
            <m3e-app-bar size="small" class="page-appbar">
              <m3e-icon-button slot="leading" class="mobile-menu" :aria-label="t('common.menu')" @click="mobileNavOpen = !mobileNavOpen">
                <m3e-icon name="menu"></m3e-icon>
              </m3e-icon-button>
              <div slot="trailing" class="appbar-trailing">
                <label class="search">
                  <m3e-icon name="search"></m3e-icon>
                  <input v-model="query" :placeholder="t('common.search')" />
                </label>
                <m3e-toolbar variant="standard" shape="rounded" class="quick-toolbar">
                  <m3e-icon-button variant="tonal" :aria-label="t('common.refresh')"><m3e-icon name="refresh"></m3e-icon></m3e-icon-button>
                  <m3e-icon-button :aria-label="t('common.sync')"><m3e-icon name="sync"></m3e-icon></m3e-icon-button>
                  <m3e-icon-button :aria-label="t('common.upload')"><m3e-icon name="upload"></m3e-icon></m3e-icon-button>
                </m3e-toolbar>
                <div class="account">
                  <span>{{ snapshot.profile.email.slice(0, 1).toUpperCase() }}</span>
                  <div>
                    <strong>{{ snapshot.profile.email }}</strong>
                    <small>{{ snapshot.profile.role }}</small>
                  </div>
                </div>
                <m3e-icon-button :aria-label="t('common.signOut')" @click="authenticated = false"><m3e-icon name="logout"></m3e-icon></m3e-icon-button>
              </div>
            </m3e-app-bar>

            <section class="page-heading">
              <div>
                <h1>{{ t(activeModule.navKey) }}</h1>
              </div>
              <m3e-button class="primary-action" variant="filled">
                <m3e-icon slot="icon" :name="action.icon"></m3e-icon>
                {{ t(action.labelKey) }}
              </m3e-button>
            </section>

            <section v-if="activeModule.id === 'dashboard'" class="metrics">
              <m3e-card v-for="metric in snapshot.metrics" :key="metric.labelKey" variant="filled" class="motion-card metric-card">
                <div slot="content" class="metric">
                  <span :class="stateClass(metric.state)"><m3e-icon :name="metric.state === 'healthy' ? 'check_circle' : 'warning'"></m3e-icon></span>
                  <p>{{ t(metric.labelKey) }}</p>
                  <strong>{{ metric.value }}</strong>
                  <small>{{ t(metric.detailKey) }}</small>
                </div>
              </m3e-card>
            </section>

            <section v-if="activeModule.id === 'sync'" class="source-grid">
              <m3e-card v-for="source in snapshot.syncSources" :key="source.id" variant="filled" class="motion-card">
                <div slot="content" class="source-card">
                  <div class="row-title">
                    <span class="icon-tile"><m3e-icon name="cloud_sync"></m3e-icon></span>
                    <div>
                      <strong>{{ source.name }}</strong>
                      <small>{{ source.type }}</small>
                    </div>
                  </div>
                  <code>{{ source.rootPath }}</code>
                  <m3e-chip-set>
                    <m3e-chip v-for="item in source.include" :key="item">{{ item }}</m3e-chip>
                  </m3e-chip-set>
                  <span :class="stateClass(source.state)">{{ stateLabel(source.state) }}</span>
                </div>
              </m3e-card>
            </section>

            <section v-if="activeModule.id === 'settings'" class="settings-grid">
              <m3e-card variant="filled" class="motion-card">
                <div slot="content" class="stack">
                  <h2>{{ t("settings.server") }}</h2>
                  <label class="field"><span>{{ t("settings.serverUrl") }}</span><input :value="snapshot.profile.serverUrl" /></label>
                  <label class="field"><span>{{ t("settings.storageRoot") }}</span><input value="/var/lib/monica/vaults" /></label>
                  <label class="field"><span>{{ t("settings.sessionLifetime") }}</span><input value="30 minutes" /></label>
                  <label class="field"><span>{{ t("settings.maxUpload") }}</span><input value="512 MB" /></label>
                </div>
              </m3e-card>
              <m3e-card variant="filled" class="motion-card">
                <div slot="content" class="stack">
                  <h2>{{ t("settings.features") }}</h2>
                  <label class="switch-row">{{ t("settings.serverBackup") }} <m3e-switch checked icons="both"></m3e-switch></label>
                  <label class="switch-row">{{ t("settings.send") }} <m3e-switch checked icons="both"></m3e-switch></label>
                  <label class="switch-row">{{ t("settings.collections") }} <m3e-switch checked icons="both"></m3e-switch></label>
                  <label class="switch-row">{{ t("settings.mdbxRelay") }} <m3e-switch checked icons="both"></m3e-switch></label>
                  <label class="switch-row">{{ t("settings.smtp") }} <m3e-switch icons="both"></m3e-switch></label>
                </div>
              </m3e-card>
              <AppearancePanel class="motion-card" />
            </section>

            <section v-else class="content-grid" :class="{ compact: activeModule.compact, 'with-detail': showDetail }">
              <m3e-card variant="filled" class="data-card motion-card">
                <div slot="header" class="card-head">
                  <div>
                    <h2>{{ t(activeModule.titleKey) }}</h2>
                  </div>
                </div>
                <DataTable :columns="activeModule.columns" :rows="rows" />
              </m3e-card>

              <ProjectDetail
                v-if="showDetail && selectedProject"
                :project="selectedProject"
                :entries="selectedEntries"
                :attachments="selectedAttachments"
                class="motion-card"
              />
            </section>
          </div>
        </Transition>
      </main>
    </div>
  </m3e-theme>
</template>
