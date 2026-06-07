<script setup lang="ts">
const emit = defineEmits<{
  key: [value: string]
}>()

const keys = [
  { label: 'Tab', key: '\t' },
  { label: 'Ctrl+C', key: '\x03' },
  { label: 'Ctrl+D', key: '\x04' },
  { label: 'Ctrl+L', key: '\x0c' },
  { label: 'Ctrl+Z', key: '\x1a' },
  { label: 'Ctrl+A', key: '\x01' },
  { label: 'Ctrl+E', key: '\x05' },
  { label: 'Esc', key: '\x1b' },
  { label: '↑', key: '\x1b[A' },
  { label: '↓', key: '\x1b[B' },
  { label: '←', key: '\x1b[D' },
  { label: '→', key: '\x1b[C' },
  { label: 'Home', key: '\x1b[H' },
  { label: 'End', key: '\x1b[F' },
  { label: 'PgUp', key: '\x1b[5~' },
  { label: 'PgDn', key: '\x1b[6~' },
  { label: 'F1', key: '\x1bOP' },
  { label: 'F2', key: '\x1bOQ' },
  { label: 'F3', key: '\x1bOR' },
  { label: 'F4', key: '\x1bOS' },
]
</script>

<template>
  <div class="virtual-keyboard">
    <button v-for="k in keys" :key="k.label" @click="emit('key', k.key)">
      {{ k.label }}
    </button>
  </div>
</template>
