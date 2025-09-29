<script setup lang="ts">
import { onMounted, ref } from 'vue'

const props = defineProps<{
  label: string
  disabled?: boolean
  keybind?: string
}>()

const btn = ref<HTMLElement | null>(null)

if (props.keybind) {
  onMounted(() => {
    window.addEventListener('keydown', (e) => {
      if (btn.value) {
        if (e.key.toLowerCase() === props.keybind?.toLowerCase()) {
          if (!props.disabled) {
            const button = btn.value
            button?.click()
          }
        }
      }
    })
  })
}
</script>

<template>
  <button :disabled="props.disabled" ref="btn">
    {{ props.label }}
  </button>
</template>

<style scoped>
button {
  border: 1px solid #bbb;
  padding: 16px;
  background: white;
  font-size: 24px;
  margin: 0 16px;
  font-family: 'Times New Roman', 'Times', serif;
  cursor: pointer;
}

button:disabled {
  cursor: not-allowed;
  color: #bbb;
}
</style>
