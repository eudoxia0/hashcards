// Copyright 2025 Fernando Borretti
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

/// A minimal, zero-dependency, completely insecure PRNG to shuffle the cards.
pub struct TinyRng {
    state: u64,
}

const A: u64 = 6364136223846793005;
const C: u64 = 1442695040888963407;

impl TinyRng {
    /// Initialize the RNG from a seed.
    pub fn from_seed(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u32(&mut self) -> u32 {
        let new = self.state.wrapping_mul(A).wrapping_add(C);
        self.state = new;
        (new >> 32) as u32
    }

    // Generate random number in range [0, max).
    pub fn generate(&mut self, max: u32) -> u32 {
        self.next_u32() % max
    }
}

pub fn shuffle<T>(v: Vec<T>, rng: &mut TinyRng) -> Vec<T> {
    let mut v = v;
    let len = v.len() as u32;
    for i in 0..len {
        let j = rng.generate(len);
        v.swap(i as usize, j as usize);
    }
    v
}
