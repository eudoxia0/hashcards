<script setup lang="ts">
import { computed, ref, type ComputedRef, type Ref } from 'vue'
import Button from './Button.vue'
import Spacer from './Spacer.vue'

interface BasicCard {
  hash: string
  kind: 'Basic'
  deckName: string
  question: string
  answer: string
}

interface ClozeCard {
  hash: string
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

const cards: Ref<CardData[]> = ref([])
const reveal: Ref<boolean> = ref(false)
const cardsDone: Ref<number> = ref(0)
const cardIndex: Ref<number> = ref(0)
const totalCards: Ref<number> = ref(0)

const currentCard: ComputedRef<CardData | null> = computed(() => {
  if (cards.value.length === 0) {
    return null
  }
  return cards.value[cardIndex.value]
})

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
  reveal.value = false
  cardsDone.value += 1
  cards.value.splice(cardIndex.value, 1)
}

function finish() {
  console.log('Aborted review')
  cards.value = []
}

// Mimic API calls:
cards.value = [
  {
    hash: 'a',
    kind: 'Basic',
    deckName: 'Geography',
    question: '<p>What is the capital of Germany?</p>',
    answer: '<p>Berlin</p>',
  },
  {
    hash: 'b',
    kind: 'Basic',
    deckName: 'Geography',
    question: '<p>What is the capital of Paris?</p>',
    answer: '<p>France</p>',
  },
  {
    hash: 'c',
    kind: 'Cloze',
    deckName: 'Chemistry',
    prompt: '<p>The atomic number of lithium is <span class="cloze">.............</span>.</p>',
    answer: '<p>The atomic number of lithium is <span class="cloze-reveal">3</span>.</p>',
  },
]

totalCards.value = cards.value.length
</script>

<template>
  <div v-if="currentCard" class="root">
    <div class="controls">
      <Button label="<" @click="prevCard" />
      <Button label=">" @click="nextCard" />
      <Spacer />
      <Button label="Undo" />
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
      }
    }

    .content {
    }
  }
}
</style>
