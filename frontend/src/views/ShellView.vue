<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import Terminal from '@/components/Terminal.vue'
import VirtualKeyboard from '@/components/VirtualKeyboard.vue'
import ChangePassword from '@/components/ChangePassword.vue'
import FileUpload from '@/components/FileUpload.vue'

const router = useRouter()
const auth = useAuthStore()

const terminalRef = ref<InstanceType<typeof Terminal> | null>(null)
const showKeyboard = ref(false)
const showChangePassword = ref(false)
const showFileUpload = ref(false)

function sendKey(key: string) {
  terminalRef.value?.sendKey(key)
}

function logout() {
  terminalRef.value?.disconnect()
  auth.logout()
  router.push({ name: 'login' })
}
</script>

<template>
  <div class="shell-page">
    <div class="toolbar">
      <span class="user-info">{{ auth.username }}</span>
      <button @click="showKeyboard = !showKeyboard">Keyboard</button>
      <button @click="showChangePassword = true">Password</button>
      <button @click="showFileUpload = true">Upload</button>
      <button @click="logout" class="logout-btn">Logout</button>
    </div>

    <VirtualKeyboard v-if="showKeyboard" @key="sendKey" />

    <Terminal ref="terminalRef" />

    <ChangePassword
      v-if="showChangePassword"
      @close="showChangePassword = false"
    />
    <FileUpload
      v-if="showFileUpload"
      @close="showFileUpload = false"
    />
  </div>
</template>
