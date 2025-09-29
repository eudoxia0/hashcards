<script setup lang="ts">
import { computed, ref, type ComputedRef, type Ref } from 'vue'
import Button from './Button.vue'
import Spacer from './Spacer.vue'

interface BasicCard {
  kind: 'Basic'
  deckName: string
  question: string
  answer: string
}

interface ClozeCard {
  kind: 'Cloze'
  deckName: string
  prompt: string
  answer: string
}

type CardData = BasicCard | ClozeCard

enum Grade {
  FORGOT = 'forgot',
  REMEMBERED = 'remembered',
}

const cards: Ref<CardData[]> = ref([
  {
    kind: 'Basic',
    deckName: 'Geography',
    question: '<p>What is the capital of Germany?</p>',
    answer: '<p>Berlin</p>',
  },
  {
    kind: 'Basic',
    deckName: 'Geography',
    question: '<p>What is the capital of Paris?</p>',
    answer: '<p>France</p>',
  },
  {
    kind: 'Cloze',
    deckName: 'Chemistry',
    prompt: '<p>The atomic number of lithium is <span class="cloze">.............</span>.</p>',
    answer: '<p>The atomic number of lithium is <span class="cloze-reveal">3</span>.</p>',
  },
])

const reveal: Ref<boolean> = ref(false)

const cardsDone: number = 0
const cardIndex: Ref<number> = ref(0)
const totalCards: ComputedRef<number> = computed(() => cards.value.length)
const currentCard: ComputedRef<CardData> = computed(() => cards.value[cardIndex.value])

const isFinished: Ref<boolean> = ref(false)

function prevCard() {
  reveal.value = false
  if (cardIndex.value > 0) {
    cardIndex.value -= 1
  } else {
    cardIndex.value = cards.value.length - 1
  }
}

function nextCard() {
  reveal.value = false
  if (cardIndex.value < cards.value.length - 1) {
    cardIndex.value += 1
  } else {
    cardIndex.value = 0
  }
}

function review(grade: Grade) {
  console.log(`Reviewed card ${cardIndex.value} with grade ${grade}`)
  nextCard()
}

function finish() {
  console.log('Finished reviewing')
  isFinished.value = true
}
</script>

<template>
  <div v-if="isFinished" class="root">Session Completed</div>
  <div v-else class="root">
    <div class="header">
      <h1>{{ currentCard.deckName }}</h1>
      <Spacer />
      <Button label="<" @click="prevCard" />
      <Button label=">" @click="nextCard" />
      <Spacer />
      <div class="progress">{{ cardsDone }} / {{ totalCards }}</div>
    </div>
    <div class="content">
      <template v-if="currentCard.kind === 'Basic'">
        <div class="question" v-html="currentCard.question" />
        <div class="answer">
          <div v-if="reveal" v-html="currentCard.answer" />
        </div>
      </template>
      <template v-else>
        <div v-if="reveal" class="prompt" v-html="currentCard.answer" />
        <div v-else class="prompt" v-html="currentCard.prompt" />
      </template>
    </div>
    <div class="controls">
      <Button label="Undo" />
      <Spacer />
      <Button v-if="!reveal" label="Reveal" @click="reveal = true" />
      <Button v-if="reveal" label="Forgot" @click="review(Grade.FORGOT)" />
      <Button v-if="reveal" label="Remembered" @click="review(Grade.REMEMBERED)" />
      <Spacer />
      <Button label="End" @click="finish()" />
    </div>
  </div>
</template>

<style scoped>
.root {
  width: 100vw;
  height: 100vh;
  display: flex;
  flex-direction: column;
}

.content {
  flex: 1;
}

.header,
.prompt,
.question,
.answer,
.controls {
  padding: 32px;
}

.header,
.question,
.answer,
.prompt {
  width: 100%;
  border-bottom: 1px solid black;
}

.header {
  display: flex;
  flex-direction: row;
  justify-content: space-between;
  align-items: center;

  h1 {
    font-size: 36px;
    font-weight: 300;
    flex: 1;
  }

  .progress {
    font-size: 24px;
  }
}

.question,
.answer,
.prompt {
  overflow-y: auto;
}

.question {
  height: 75%;
}

.answer {
  height: 25%;
}

.prompt {
  height: 100%;
}

.controls {
  display: flex;
  flex-direction: row;
  justify-content: center;
}
</style>
