import type { CardData } from './types'

export const CARDS: CardData[] = [
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
