<script setup lang="ts">
const deckName = 'Geography'
const cardsDone = 0
const totalCards = 7
</script>

<template>
  <div class="root">
    <div class="card">
      <div class="header">
        <h1>{{ deckName }}</h1>
        <div class="progress">{{ cardsDone }} / {{ totalCards }}</div>
      </div>
      <div class="content">
        <div class="prompt rich-text">
          <p>Berlin is the capital of <span class="cloze">.............</span>.</p>
        </div>
      </div>
      <div class="controls">
        <form action="/" method="post">
          <input id="undo" type="submit" name="action" value="Undo" />
          <div class="spacer"></div>
          <input id="reveal" type="submit" name="action" value="Reveal" />
          <div class="spacer"></div>
          <input id="end" type="submit" name="action" value="End" />
        </form>
      </div>
    </div>
  </div>
</template>

<style scoped>
.root,
.card {
  width: 100vw;
  height: 100vh;
}

.root {
  max-height: 100vh;
}

.card {
  display: flex;
  flex-direction: column;
}

.content {
  flex: 1;
}

.header,
.question,
.answer,
.prompt,
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

.controls form {
  display: flex;
  flex-direction: row;
  justify-content: center;
}

input {
  border: 1px solid #bbb;
  padding: 16px;
  background: white;
  font-size: 24px;
  margin: 0 16px;
  font-family: 'Times New Roman', 'Times', serif;
  cursor: pointer;
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

.finished {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;

  h1 {
    font-size: 48px;
    font-weight: bold;
  }
}

.spacer {
  flex: 1;
}
</style>
