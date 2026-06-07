<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue'
import { Terminal as XTerminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import '@xterm/xterm/css/xterm.css'
import { useAuthStore } from '@/stores/auth'

const auth = useAuthStore()
const termEl = ref<HTMLDivElement | null>(null)

let term: XTerminal | null = null
let fitAddon: FitAddon | null = null
let ws: WebSocket | null = null

function sendKey(key: string) {
  if (ws?.readyState === WebSocket.OPEN) {
    ws.send(new TextEncoder().encode(key))
  }
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
  disconnect()
})

defineExpose({ sendKey, disconnect })
</script>

<template>
  <div ref="termEl" class="terminal-container"></div>
</template>
