<script setup lang="ts">
import renderMathInElement from 'katex/dist/contrib/auto-render'
import 'katex/dist/katex.min.css'
import { onMounted, ref } from 'vue'

const props = defineProps<{
  html: string
}>()

const math = ref<HTMLElement | null>(null)

onMounted(() => {
  if (math.value) {
    renderMathInElement(math.value, {
      delimiters: [
        { left: '$', right: '$', display: false },
        { left: '$$', right: '$$', display: true },
      ],
      throwOnError: false,
    })
  }
})
</script>

<template>
  <div class="rich-text" ref="math">
    <div v-html="props.html" />
  </div>
</template>

<style scoped>
.rich-text {
  font-size: 28px;
}
</style>
