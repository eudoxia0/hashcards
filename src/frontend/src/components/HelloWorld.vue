<script setup lang="ts">
import { useCounterStore } from '@/stores/counter'

const counter = useCounterStore()
</script>

<template>
  <h1>Hello, world!</h1>
  Counter: {{ counter.count }}
  <button @click="counter.increment">Increment</button>
</template>
