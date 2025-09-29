<script setup lang="ts">
import { computed, ref, type ComputedRef, type Ref } from 'vue'
import Button from '@/components/Button.vue'
import Spacer from '@/components/Spacer.vue'
import RichText from '@/components/RichText.vue'
import { Grade, type CardData, type Review } from './types'

/// The stack of cards to review.
const cards: Ref<CardData[]> = ref([])
/// The total number of cards in the session.
const totalCards: Ref<number> = ref(0)
/// Whether or not to show the answer.
const reveal: Ref<boolean> = ref(false)
/// The list of card reviews made.
const reviews: Ref<Review[]> = ref([])
/// Are we done?
const done: Ref<boolean> = ref(false)

/// The current card, or null if there are no cards.
const currentCard: ComputedRef<CardData | null> = computed(() => {
  if (cards.value.length === 0) {
    return null
  }
  return cards.value[0]
})

/// The number of graded cards.
const cardsDone: ComputedRef<number> = computed(() => {
  return totalCards.value - cards.value.length
})

/// Should the undo button be disabled?
const undoDisabled: ComputedRef<boolean> = computed(() => {
  return reviews.value.length === 0
})

/// Assign a grade to the current card.
function review(grade: Grade) {
  if (currentCard.value) {
    reveal.value = false
    console.log(`Card ${currentCard.value?.hash} graded as ${grade}`)
    reviews.value.push({
      card: currentCard.value,
      grade,
    })
    if (grade === Grade.FORGOT) {
      // Put the card at the back of the stack.
      const card = cards.value.shift()
      if (card) {
        cards.value.push(card)
      }
    } else {
      // Remove the card.
      cards.value.shift()
    }
    if (cards.value.length === 0) {
      done.value = true
    }
  }
}

/// Finish the session early.
function finish() {
  done.value = true
}

/// Undo the last action.
function undo() {
  if (reviews.value.length === 0) {
    //
    return
  }
  const lastReview = reviews.value.pop()
  if (lastReview) {
    if (lastReview.grade === Grade.FORGOT) {
      // Take the card from the back of the stack
      cards.value.pop()
    }
    cards.value.unshift(lastReview.card)
    done.value = false
    reveal.value = false
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
  },
  {
    hash: 'b',
    kind: 'Basic',
    deckName: 'Geography',
    question: '<p>Who wrote <i>The Tempest</i>?</p>',
    answer: '<p>Shakespeare</p>',
  },
  {
    hash: 'c',
    kind: 'Cloze',
    deckName: 'Chemistry',
    prompt: '<p>The atomic number of lithium is <span class="cloze">.............</span>.</p>',
    answer: '<p>The atomic number of lithium is <span class="cloze-reveal">3</span>.</p>',
  },
  {
    hash: 'd',
    kind: 'Basic',
    deckName: 'Math',
    question: '<p>What does $2+2$ equal?</p>',
    answer: '<p>$4$</p>',
  },
]

totalCards.value = cards.value.length
</script>

<template>
  <div v-if="!done && currentCard" class="root">
    <div class="controls">
      <Button label="Undo" :disabled="undoDisabled" @click="undo()" />
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
          <RichText :html="currentCard.question" />
          <RichText v-if="reveal" :html="currentCard.answer" />
        </template>
        <template v-else>
          <RichText v-if="reveal" :html="currentCard.answer" />
          <RichText v-else :html="currentCard.prompt" />
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
