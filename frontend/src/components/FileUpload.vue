<script setup lang="ts">
import { ref } from 'vue'
import { uploadFile } from '@/api/upload'

const emit = defineEmits<{ close: [] }>()

const file = ref<File | null>(null)
const error = ref('')
const success = ref('')
const loading = ref(false)

function onFileChange(e: Event) {
  const input = e.target as HTMLInputElement
  file.value = input.files?.[0] || null
}

async function submit() {
  if (!file.value) return
  error.value = ''
  success.value = ''
  loading.value = true
  try {
    await uploadFile(file.value)
    success.value = `File uploaded: ${file.value.name}`
    file.value = null
  } catch (e: any) {
    error.value = e.response?.data?.error || 'Upload failed'
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <div class="modal-overlay" @click.self="emit('close')">
    <div class="modal">
      <h2>Upload File</h2>
      <form @submit.prevent="submit">
        <div class="form-group">
          <input type="file" @change="onFileChange" required />
        </div>
        <p v-if="error" class="error">{{ error }}</p>
        <p v-if="success" class="success">{{ success }}</p>
        <div class="modal-actions">
          <button type="button" @click="emit('close')">Cancel</button>
          <button type="submit" :disabled="loading || !file">
            {{ loading ? 'Uploading...' : 'Upload' }}
          </button>
        </div>
      </form>
    </div>
  </div>
</template>
