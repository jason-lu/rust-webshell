<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue'
import { Terminal as XTerminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import '@xterm/xterm/css/xterm.css'
import { useAuthStore } from '@/stores/auth'

const auth = useAuthStore()
const termEl = ref<HTMLDivElement | null>(null)

// Context menu state
const menuVisible = ref(false)
const menuX = ref(0)
const menuY = ref(0)

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

function disconnect() {
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
    if (term && fitAddon) {
      fitAddon.fit()
      const dims = fitAddon.proposeDimensions()
      if (dims) {
        ws!.send(new TextEncoder().encode(JSON.stringify({ cols: dims.cols, rows: dims.rows })))
      }
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
    term?.write('\r\n\x1b[31m[Connection closed]\x1b[0m\r\n')
  }

  ws.onerror = () => {
    auth.logout()
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
  <div ref="termEl" class="terminal-container"></div>
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
