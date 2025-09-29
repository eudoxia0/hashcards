<script setup lang="ts">
import { computed, ref, type ComputedRef, type Ref } from 'vue'
import Button from './Button.vue'
import Spacer from './Spacer.vue'

enum Grade {
  FORGOT = 'forgot',
  REMEMBERED = 'remembered',
}

interface BasicCard {
  hash: string
  kind: 'Basic'
  deckName: string
  question: string
  answer: string
  grade: Grade | null
}

interface ClozeCard {
  hash: string
  kind: 'Cloze'
  deckName: string
  prompt: string
  answer: string
  grade: Grade | null
}

type CardData = BasicCard | ClozeCard

/// The list of all cards.
const cards: Ref<CardData[]> = ref([])
/// The total number of cards in the session.
const totalCards: Ref<number> = ref(0)
/// Whether or not to show the answer.
const reveal: Ref<boolean> = ref(false)
/// The index of the current card.
const cardIndex: Ref<number> = ref(0)

/// The card at the current index, or null if there are no cards.
const currentCard: ComputedRef<CardData | null> = computed(() => {
  if (cards.value.length === 0) {
    return null
  }
  return cards.value[cardIndex.value]
})

/// The number of graded cards.
const cardsDone: ComputedRef<number> = computed(() => {
  return cards.value.filter((card) => card.grade !== null).length
})

/// Assign a grade to the current card, and move to the next one.
function review(grade: Grade) {
  reveal.value = false
  if (currentCard.value) {
    currentCard.value.grade = grade
  }
  cardIndex.value += 1
}

/// Finish the session early.
function finish() {
  console.log('Aborted review')
}

/// Undo the last grading action.
function undo() {
  if (cardIndex.value > 0) {
    cardIndex.value -= 1
    if (currentCard.value) {
      currentCard.value.grade = null
    }
  }
}

// Mimic API calls:
cards.value = [
  {
    hash: 'a',
    kind: 'Basic',
    deckName: 'Geography',
    question: '<p>What is the capital of Germany?</p>',
    answer: '<p>Berlin</p>',
    grade: null,
  },
  {
    hash: 'b',
    kind: 'Basic',
    deckName: 'Geography',
    question: '<p>Who wrote <i>The Tempest</i>?</p>',
    answer: '<p>Shakespeare</p>',
    grade: null,
  },
  {
    hash: 'c',
    kind: 'Cloze',
    deckName: 'Chemistry',
    prompt: '<p>The atomic number of lithium is <span class="cloze">.............</span>.</p>',
    answer: '<p>The atomic number of lithium is <span class="cloze-reveal">3</span>.</p>',
    grade: null,
  },
]

totalCards.value = cards.value.length
</script>

<template>
  <div v-if="currentCard" class="root">
    <div class="controls">
      <Button label="Undo" :disabled="cardIndex === 0" @click="undo()" />
      <Spacer />
      <Button v-if="!reveal" label="Reveal" @click="reveal = true" />
      <Button v-if="reveal" label="Forgot" @click="review(Grade.FORGOT)" />
      <Button v-if="reveal" label="Remembered" @click="review(Grade.REMEMBERED)" />
      <Spacer />
      <Button label="End" @click="finish()" />
      <div class="progress">{{ cardsDone }} / {{ totalCards }}</div>
    </div>
    <div class="card">
      <div class="header">
        <h1>{{ currentCard.deckName }}</h1>
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
    </div>
  </div>
  <div v-else class="root">Session Completed</div>
</template>

<style scoped>
.root {
  width: 100vw;
  height: 100vh;
  display: flex;
  flex-direction: column;

  .controls {
    display: flex;
    flex-direction: row;
    justify-content: start;
    align-items: center;

    .progress {
      font-size: 32px;
    }
  }

  .card {
    padding: 32px;
    border: 1px solid black;

    header {
      h1 {
        font-size: 36px;
      }
    }
  }
}
</style>
