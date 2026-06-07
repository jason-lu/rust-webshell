<script setup lang="ts">
import { ref } from 'vue'
import { useAuthStore } from '@/stores/auth'
import { changePassword } from '@/api/auth'

const emit = defineEmits<{ close: [] }>()
const auth = useAuthStore()

const oldPassword = ref('')
const newPassword = ref('')
const error = ref('')
const success = ref('')
const loading = ref(false)

async function submit() {
  error.value = ''
  success.value = ''
  loading.value = true
  try {
    await changePassword(auth.username, oldPassword.value, newPassword.value)
    success.value = 'Password changed successfully'
    oldPassword.value = ''
    newPassword.value = ''
  } catch (e: any) {
    error.value = e.response?.data?.error || 'Failed to change password'
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <div class="modal-overlay" @click.self="emit('close')">
    <div class="modal">
      <h2>Change Password</h2>
      <form @submit.prevent="submit">
        <div class="form-group">
          <label>Old Password</label>
          <input v-model="oldPassword" type="password" required />
        </div>
        <div class="form-group">
          <label>New Password</label>
          <input v-model="newPassword" type="password" required />
        </div>
        <p v-if="error" class="error">{{ error }}</p>
        <p v-if="success" class="success">{{ success }}</p>
        <div class="modal-actions">
          <button type="button" @click="emit('close')">Cancel</button>
          <button type="submit" :disabled="loading">
            {{ loading ? 'Saving...' : 'Save' }}
          </button>
        </div>
      </form>
    </div>
  </div>
</template>
