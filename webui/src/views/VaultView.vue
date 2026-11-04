<script setup lang="ts">
import { computed, ref, watch } from "vue";
import ItemDetail from "../components/ItemDetail.vue";
import ItemEditor from "../components/ItemEditor.vue";
import ItemIcon from "../components/ItemIcon.vue";
import { t } from "../i18n";
import { formatRelative } from "../lib/format";
import { creatableTypes, itemSubtitle, searchText, typeByKey } from "../lib/items";
import { type Route, go, searchQuery } from "../lib/router";
import { confirm, toastError } from "../lib/ui";
import { deleteFolder, emptyTrash, renameFolder, vault, type VaultItem } from "../lib/vault";

const props = defineProps<{ route: Extract<Route, { name: "vault" }> }>();

const selectedId = ref<string | null>(null);
const editing = ref<"new" | "edit" | null>(null);
const newType = ref(1);
const sort = ref<"name" | "updated">("name");

const folder = computed(() => (props.route.filter === "folder" ? vault.folders.find((f) => f.id === props.route.folderId) : undefined));
const filterType = computed(() => typeByKey(props.route.filter));

const title = computed(() => {
  const filter = props.route.filter;
  if (folder.value) return folder.value.name;
  if (filterType.value) return t(filterType.value.labelKey);
  if (filter === "favorites") return t("nav.favorites");
  if (filter === "archive") return t("nav.archive");
  if (filter === "trash") return t("nav.trash");
  return t("nav.allItems");
});

const list = computed(() => {
  const filter = props.route.filter;
  const query = searchQuery.value.trim().toLowerCase();
  const items = vault.items.filter((item) => {
    if (filter === "trash") return item.trashed;
    if (item.trashed) return false;
    if (filter === "archive") return item.archived;
    if (item.archived && !query) return false;
    if (filter === "favorites") return item.favorite;
    if (filter === "folder") return item.folderId === props.route.folderId;
    if (filterType.value) return item.type === filterType.value.id;
    return true;
  });
  const matched = query ? items.filter((item) => searchText(item.payload).includes(query)) : items;
  return [...matched].sort((a, b) =>
    sort.value === "updated" ? b.revisionDate.localeCompare(a.revisionDate) : a.payload.name.localeCompare(b.payload.name)
  );
});

const selected = computed(() => vault.items.find((item) => item.id === selectedId.value) ?? null);
const canCreate = computed(() => props.route.filter !== "trash" && props.route.filter !== "archive");

watch(
  () => [props.route.filter, props.route.folderId],
  () => {
    if (editing.value !== "new") editing.value = null;
    if (selectedId.value && !list.value.some((i) => i.id === selectedId.value)) selectedId.value = null;
  }
);

function select(item: VaultItem) {
  if (editing.value && !window.confirm(t("item.discardChanges"))) return;
  editing.value = null;
  selectedId.value = item.id;
}

function startNew() {
  newType.value = filterType.value && filterType.value.key !== "passkey" ? filterType.value.id : 1;
  selectedId.value = null;
  editing.value = "new";
}

function onSaved(item: VaultItem) {
  editing.value = null;
  selectedId.value = item.id;
}

function onGone() {
  selectedId.value = null;
  editing.value = null;
}

async function clearTrash() {
  if (!(await confirm(t("trash.emptyTitle"), t("trash.emptyBody", { count: list.value.length }), { danger: true, confirmLabel: t("trash.empty") }))) return;
  try {
    await emptyTrash();
    selectedId.value = null;
  } catch (e) {
    toastError(e);
  }
}

async function renameCurrentFolder() {
  if (!folder.value) return;
  const name = window.prompt(t("folder.rename"), folder.value.name)?.trim();
  if (!name || name === folder.value.name) return;
  try {
    await renameFolder(folder.value.id, name);
  } catch (e) {
    toastError(e);
  }
}

async function deleteCurrentFolder() {
  if (!folder.value) return;
  if (!(await confirm(t("folder.deleteTitle"), t("folder.deleteBody", { name: folder.value.name }), { danger: true, confirmLabel: t("common.delete") }))) return;
  try {
    await deleteFolder(folder.value.id);
    go("vault/all");
  } catch (e) {
    toastError(e);
  }
}

const emptyIcon = computed(() => (props.route.filter === "trash" ? "delete" : props.route.filter === "archive" ? "archive" : filterType.value?.icon ?? "inventory_2"));
</script>

<template>
  <div class="vault" :class="{ 'has-detail': selected || editing }">
    <section class="list-pane">
      <header class="pane-header">
        <div class="pane-title">
          <h1>{{ title }}</h1>
          <span class="count">{{ list.length }}</span>
        </div>
        <div class="pane-actions">
          <m3e-icon-button
            :aria-label="t('list.sort')"
            :title="sort === 'name' ? t('list.sortName') : t('list.sortUpdated')"
            @click="sort = sort === 'name' ? 'updated' : 'name'"
          >
            <m3e-icon :name="sort === 'name' ? 'sort_by_alpha' : 'schedule'"></m3e-icon>
          </m3e-icon-button>
          <template v-if="folder">
            <m3e-icon-button :aria-label="t('folder.rename')" @click="renameCurrentFolder"><m3e-icon name="edit"></m3e-icon></m3e-icon-button>
            <m3e-icon-button :aria-label="t('common.delete')" @click="deleteCurrentFolder"><m3e-icon name="folder_delete"></m3e-icon></m3e-icon-button>
          </template>
          <m3e-button v-if="route.filter === 'trash' && list.length" variant="text" @click="clearTrash">{{ t("trash.empty") }}</m3e-button>
          <m3e-button v-if="canCreate" variant="filled" @click="startNew">
            <m3e-icon slot="icon" name="add"></m3e-icon>{{ t("item.new") }}
          </m3e-button>
        </div>
      </header>

      <div v-if="vault.syncing && !vault.items.length" class="list-skeleton">
        <div v-for="i in 6" :key="i" class="skeleton-row"><span></span><div><i></i><i></i></div></div>
      </div>
      <ul v-else-if="list.length" class="item-list" role="listbox">
        <li
          v-for="item in list"
          :key="item.id"
          role="option"
          :aria-selected="item.id === selectedId"
          :class="{ selected: item.id === selectedId, broken: item.error }"
          tabindex="0"
          @click="select(item)"
          @keydown.enter="select(item)"
        >
          <ItemIcon :type="item.type" :name="item.payload.name" />
          <div class="item-text">
            <strong>{{ item.payload.name || t("item.untitled") }}</strong>
            <small>{{ item.error ? t("item.decryptFailed") : itemSubtitle(item.type, item.payload) || formatRelative(item.revisionDate) }}</small>
          </div>
          <m3e-icon v-if="item.attachments.length" class="row-badge" name="attach_file"></m3e-icon>
          <m3e-icon v-if="item.favorite" class="row-badge star" name="star" filled></m3e-icon>
        </li>
      </ul>
      <div v-else class="empty-state">
        <span class="empty-icon"><m3e-icon :name="searchQuery ? 'search_off' : emptyIcon"></m3e-icon></span>
        <strong>{{ searchQuery ? t("list.noResults") : t("list.empty") }}</strong>
        <p v-if="!searchQuery && canCreate">{{ t("list.emptyHint") }}</p>
      </div>
    </section>

    <section class="detail-pane">
      <ItemEditor
        v-if="editing"
        :key="editing === 'new' ? `new-${newType}` : selected?.id"
        :item="editing === 'edit' ? selected : null"
        :initial-type="newType"
        :initial-folder="route.filter === 'folder' ? route.folderId ?? null : null"
        :initial-favorite="route.filter === 'favorites'"
        @saved="onSaved"
        @cancel="editing = null"
      />
      <ItemDetail v-else-if="selected" :key="selected.id" :item="selected" @edit="editing = 'edit'" @gone="onGone" @back="selectedId = null" />
      <div v-else class="detail-empty">
        <img src="/monica-logo.png" alt="" />
        <p>{{ t("detail.pick") }}</p>
        <div class="type-shortcuts">
          <button v-for="type in creatableTypes.slice(0, 6)" :key="type.id" type="button" @click="newType = type.id; editing = 'new'">
            <m3e-icon :name="type.icon"></m3e-icon>{{ t(type.labelKey) }}
          </button>
        </div>
      </div>
    </section>
  </div>
</template>
