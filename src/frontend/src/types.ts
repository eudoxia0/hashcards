export enum Grade {
  FORGOT = 'forgot',
  REMEMBERED = 'remembered',
}

export interface BasicCard {
  hash: string
  kind: 'Basic'
  deckName: string
  question: string
  answer: string
}

export interface ClozeCard {
  hash: string
  kind: 'Cloze'
  deckName: string
  prompt: string
  answer: string
}

export type CardData = BasicCard | ClozeCard

export interface Review {
  card: CardData
  grade: Grade
}
