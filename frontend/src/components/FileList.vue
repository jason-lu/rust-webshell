<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { listFiles, type FileEntry } from '@/api/files'
import { useAuthStore } from '@/stores/auth'

const emit = defineEmits<{ close: [] }>()
const auth = useAuthStore()

const entries = ref<FileEntry[]>([])
const currentPath = ref('')
const loading = ref(false)
const error = ref('')

const breadcrumbs = computed(() => {
  if (!currentPath.value) return [{ name: 'root', path: '' }]
  const parts = currentPath.value.split('/').filter(Boolean)
  const result = [{ name: 'root', path: '' }]
  let accumulated = ''
  for (const part of parts) {
    accumulated = accumulated ? `${accumulated}/${part}` : part
    result.push({ name: part, path: accumulated })
  }
  return result
})

async function load(path?: string) {
  loading.value = true
  error.value = ''
  try {
    const res = await listFiles(path)
    entries.value = res.data
    currentPath.value = path || ''
  } catch (e: any) {
    error.value = e.response?.data?.message || 'Failed to load files'
  } finally {
    loading.value = false
  }
}

function enterDir(name: string) {
  const newPath = currentPath.value ? `${currentPath.value}/${name}` : name
  load(newPath)
}

function goTo(path: string) {
  load(path)
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`
}

async function downloadFile(name: string) {
  const filePath = currentPath.value ? `${currentPath.value}/${name}` : name
  const url = `/api/download?path=${encodeURIComponent(filePath)}&token=${encodeURIComponent(auth.token)}`
  const a = document.createElement('a')
  a.href = url
  a.download = name
  a.click()
}

onMounted(() => load())
</script>

<template>
  <div class="modal-overlay" @click.self="emit('close')">
    <div class="modal file-modal">
      <h2>Files</h2>

      <div class="breadcrumbs">
        <span
          v-for="(crumb, i) in breadcrumbs"
          :key="crumb.path"
        >
          <a href="#" @click.prevent="goTo(crumb.path)">{{ crumb.name }}</a>
          <span v-if="i < breadcrumbs.length - 1" class="sep">/</span>
        </span>
      </div>

      <p v-if="error" class="error">{{ error }}</p>

      <div v-if="loading" class="loading">Loading...</div>

      <div v-else-if="entries.length === 0" class="empty">
        No files here
      </div>

      <div v-else class="file-list">
        <div
          v-for="entry in entries"
          :key="entry.name"
          class="file-entry"
        >
          <span class="file-icon">{{ entry.is_dir ? '📁' : '📄' }}</span>
          <span
            class="file-name"
            :class="{ clickable: entry.is_dir }"
            @click="entry.is_dir ? enterDir(entry.name) : undefined"
          >
            {{ entry.name }}
          </span>
          <span class="file-size">{{ entry.is_dir ? '' : formatSize(entry.size) }}</span>
          <button
            v-if="!entry.is_dir"
            class="download-btn"
            @click="downloadFile(entry.name)"
            title="Download"
          >
            ⬇
          </button>
        </div>
      </div>

      <div class="modal-actions">
        <button @click="emit('close')">Close</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.file-modal {
  max-width: 600px;
  width: 90vw;
  max-height: 80vh;
  display: flex;
  flex-direction: column;
}
.breadcrumbs {
  margin-bottom: 12px;
  font-size: 14px;
}
.breadcrumbs a {
  color: #4fc3f7;
  text-decoration: none;
}
.breadcrumbs a:hover {
  text-decoration: underline;
}
.breadcrumbs .sep {
  color: #666;
  margin: 0 2px;
}
.file-list {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}
.file-entry {
  display: flex;
  align-items: center;
  padding: 8px 4px;
  border-bottom: 1px solid #333;
  gap: 8px;
}
.file-icon {
  flex-shrink: 0;
  font-size: 18px;
}
.file-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.file-name.clickable {
  color: #4fc3f7;
  cursor: pointer;
}
.file-name.clickable:hover {
  text-decoration: underline;
}
.file-size {
  flex-shrink: 0;
  color: #999;
  font-size: 13px;
  min-width: 70px;
  text-align: right;
}
.download-btn {
  flex-shrink: 0;
  background: none;
  border: 1px solid #555;
  border-radius: 4px;
  cursor: pointer;
  padding: 2px 8px;
  font-size: 14px;
}
.download-btn:hover {
  background: #333;
}
.empty {
  text-align: center;
  color: #999;
  padding: 24px;
}
.loading {
  text-align: center;
  color: #999;
  padding: 24px;
}
</style>
