<script setup lang="ts">
const props = defineProps<{
  label: string
  disabled?: boolean
}>()
</script>

<template>
  <button :disabled="props.disabled">
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
}
</style>
