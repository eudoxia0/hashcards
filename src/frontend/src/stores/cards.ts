import { ref, type Ref } from 'vue'
import { defineStore } from 'pinia'

export interface BasicCard {
  kind: 'Basic'
  question: string
  answer: string
}

export interface ClozeCard {
  kind: 'Cloze'
  prompt: string
  answer: string
}

export type Card = BasicCard | ClozeCard

export const useCardStore = defineStore('cards', () => {
  const cards: Ref<Card[]> = ref<Card[]>([])

  const setCards: (newCards: Card[]) => void = (newCards: Card[]) => {
    cards.value = newCards
  }

  return { cards, setCards }
})

export type CardStore = ReturnType<typeof useCardStore>
