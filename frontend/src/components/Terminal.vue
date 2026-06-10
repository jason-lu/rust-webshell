<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue'
import { useRouter } from 'vue-router'
import { Terminal as XTerminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import '@xterm/xterm/css/xterm.css'
import { useAuthStore } from '@/stores/auth'

const auth = useAuthStore()
const router = useRouter()
const termEl = ref<HTMLDivElement | null>(null)
let connected = false
let intentionalClose = false

// Context menu state
const menuVisible = ref(false)
const menuX = ref(0)
const menuY = ref(0)

// Connection status
const connStatus = ref<'connected' | 'reconnecting' | 'disconnected'>('disconnected')
const reconnectAttempt = ref(0)
const MAX_RECONNECT = 10
const BASE_DELAY = 1000
let reconnectTimer: ReturnType<typeof setTimeout> | null = null

let term: XTerminal | null = null
let fitAddon: FitAddon | null = null
let ws: WebSocket | null = null

function sendKey(key: string) {
  if (ws?.readyState === WebSocket.OPEN) {
    ws.send(new TextEncoder().encode(key))
  }
}

function showContextMenu(e: MouseEvent) {
  e.preventDefault()
  menuX.value = e.clientX
  menuY.value = e.clientY
  menuVisible.value = true
}

function hideContextMenu() {
  menuVisible.value = false
}

async function copySelection() {
  if (term?.hasSelection()) {
    await navigator.clipboard.writeText(term.getSelection())
  }
  hideContextMenu()
}

async function pasteClipboard() {
  const text = await navigator.clipboard.readText()
  if (text && ws?.readyState === WebSocket.OPEN) {
    ws.send(new TextEncoder().encode(text))
  }
  hideContextMenu()
}

function scheduleReconnect() {
  if (reconnectAttempt.value >= MAX_RECONNECT) {
    connStatus.value = 'disconnected'
    term?.write('\r\n\x1b[31m[Reconnect failed. Please refresh the page.]\x1b[0m\r\n')
    return
  }
  const delay = Math.min(BASE_DELAY * Math.pow(2, reconnectAttempt.value), 30000)
  reconnectAttempt.value++
  reconnectTimer = setTimeout(() => {
    term?.write(`\x1b[90m[Reconnect attempt ${reconnectAttempt.value}/${MAX_RECONNECT}...]\x1b[0m\r\n`)
    connectWebSocket()
  }, delay)
}

function disconnect() {
  intentionalClose = true
  if (reconnectTimer) clearTimeout(reconnectTimer)
  ws?.close()
  term?.dispose()
}

function connectWebSocket() {
  const proto = location.protocol === 'https:' ? 'wss:' : 'ws:'
  const host = location.host
  const url = `${proto}//${host}/webshell/api/ws/shell?token=${auth.token}`
  ws = new WebSocket(url)
  ws.binaryType = 'arraybuffer'

  ws.onopen = () => {
    connected = true
    reconnectAttempt.value = 0
    connStatus.value = 'connected'
    if (term && fitAddon) {
      fitAddon.fit()
      const dims = fitAddon.proposeDimensions()
      if (dims) {
        ws!.send(new TextEncoder().encode(JSON.stringify({ cols: dims.cols, rows: dims.rows })))
      }
    }
    // 清除重连提示
    if (reconnectAttempt.value > 0) {
      term?.write('\r\n\x1b[32m[Reconnected]\x1b[0m\r\n')
    }
  }

  ws.onmessage = (ev) => {
    if (ev.data instanceof ArrayBuffer) {
      term?.write(new Uint8Array(ev.data))
    } else {
      term?.write(ev.data)
    }
  }

  ws.onclose = () => {
    if (!connected) {
      // WebSocket 握手失败（很可能是 token 过期/无效，服务端返回 401）
      auth.logout()
      router.push({ name: 'login' })
      return
    }
    connected = false
    if (intentionalClose) return
    // 连接意外断开 — 尝试自动重连
    connStatus.value = 'reconnecting'
    term?.write('\r\n\x1b[33m[Connection lost, reconnecting...]\x1b[0m\r\n')
    scheduleReconnect()
  }

  ws.onerror = () => {
    if (!connected) {
      auth.logout()
      router.push({ name: 'login' })
    }
  }
}

onMounted(() => {
  if (!termEl.value) return

  term = new XTerminal({
    cursorBlink: true,
    fontSize: 14,
    theme: {
      background: '#1a1a2e',
      foreground: '#e0e0e0',
      cursor: '#e0e0e0',
    },
  })

  fitAddon = new FitAddon()
  term.loadAddon(fitAddon)
  term.open(termEl.value)
  fitAddon.fit()

  // Right-click context menu for copy/paste
  termEl.value.addEventListener('contextmenu', showContextMenu)
  document.addEventListener('click', hideContextMenu)

  term.onData((data) => {
    if (ws?.readyState === WebSocket.OPEN) {
      ws.send(new TextEncoder().encode(data))
    }
  })

  const resizeObserver = new ResizeObserver(() => {
    if (!fitAddon || !term || !ws || ws.readyState !== WebSocket.OPEN) return
    fitAddon.fit()
    const dims = fitAddon.proposeDimensions()
    if (dims) {
      ws.send(new TextEncoder().encode(JSON.stringify({ cols: dims.cols, rows: dims.rows })))
    }
  })
  resizeObserver.observe(termEl.value)

  connectWebSocket()
})

onBeforeUnmount(() => {
  termEl.value?.removeEventListener('contextmenu', showContextMenu)
  document.removeEventListener('click', hideContextMenu)
  disconnect()
})

defineExpose({ sendKey, disconnect })
</script>

<template>
  <div class="terminal-wrapper">
    <div v-if="connStatus === 'reconnecting'" class="conn-banner reconnecting">
      ⚠️ 连接已断开，正在重连...
    </div>
    <div v-if="connStatus === 'disconnected'" class="conn-banner disconnected">
      ❌ 连接已断开，请刷新页面
    </div>
    <div ref="termEl" class="terminal-container"></div>
  </div>
  <Teleport to="body">
    <div
      v-if="menuVisible"
      class="context-menu"
      :style="{ left: menuX + 'px', top: menuY + 'px' }"
    >
      <button class="context-menu-item" @click="copySelection">
        <span class="shortcut">Ctrl+Insert</span> Copy
      </button>
      <button class="context-menu-item" @click="pasteClipboard">
        <span class="shortcut">Shift+Insert</span> Paste
      </button>
    </div>
  </Teleport>
</template>

<style scoped>
.terminal-wrapper {
  position: relative;
  width: 100%;
  height: 100%;
}

.terminal-container {
  width: 100%;
  height: 100%;
}

.conn-banner {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  z-index: 10;
  text-align: center;
  padding: 6px 12px;
  font-size: 13px;
  font-family: system-ui, sans-serif;
  animation: slideDown 0.3s ease-out;
}

.conn-banner.reconnecting {
  background: rgba(255, 193, 7, 0.9);
  color: #333;
}

.conn-banner.disconnected {
  background: rgba(220, 53, 69, 0.9);
  color: #fff;
}

@keyframes slideDown {
  from { transform: translateY(-100%); opacity: 0; }
  to { transform: translateY(0); opacity: 1; }
}

.context-menu {
  position: fixed;
  background: #2d2d44;
  border: 1px solid #444;
  border-radius: 4px;
  padding: 4px 0;
  min-width: 180px;
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.5);
  z-index: 9999;
}

.context-menu-item {
  display: block;
  width: 100%;
  padding: 8px 16px;
  background: none;
  border: none;
  color: #e0e0e0;
  font-size: 13px;
  text-align: left;
  cursor: pointer;
}

.context-menu-item:hover {
  background: #3a3a5c;
}

.context-menu-item .shortcut {
  float: right;
  color: #888;
  font-size: 12px;
}
</style>
