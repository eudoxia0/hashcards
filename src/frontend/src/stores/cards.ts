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

export type CardData = BasicCard | ClozeCard

export const useCardStore = defineStore('cards', () => {
  const cards: Ref<CardData[]> = ref<CardData[]>([])

  const setCards: (newCards: CardData[]) => void = (newCards: CardData[]) => {
    cards.value = newCards
  }

  return { cards, setCards }
})

export type CardStore = ReturnType<typeof useCardStore>
