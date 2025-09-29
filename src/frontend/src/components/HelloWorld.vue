<script setup lang="ts">
import Button from './Button.vue'
import Spacer from './Spacer.vue'

const deckName = 'Geography'

const cards = [
  {
    kind: 'Basic',
    question: '<p>What is the capital of Germany?</p>',
    answer: '<p>Berlin</p>',
  },
]

const cardsDone = 0
const totalCards = cards.length
const currentCard = cards[0]
</script>

<template>
  <div class="root">
    <div class="header">
      <h1>{{ deckName }}</h1>
      <div class="progress">{{ cardsDone }} / {{ totalCards }}</div>
    </div>
    <div class="content">
      <div class="question rich-text" v-html="currentCard.question" />
      <div class="answer rich-text" v-html="currentCard.answer" />
    </div>
    <div class="controls">
      <Button label="Undo" />
      <Spacer />
      <Button label="Reveal" />
      <Spacer />
      <Button label="End" />
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

.rich-text {
  font-size: 30px;

  img {
    max-width: 70%;
    display: block;
    margin: 8px auto;
  }
}

.controls {
  display: flex;
  flex-direction: row;
  justify-content: center;
}

.cloze {
  width: 30px;
  background: #a0a0a0;
  text-decoration: none;
  border-radius: 8px;
  color: transparent;
}

.cloze-reveal {
  color: royalblue;
}
</style>
