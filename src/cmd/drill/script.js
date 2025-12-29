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

document.addEventListener("DOMContentLoaded", function () {
  renderMathInElement(document.body, {
    delimiters: [
      { left: "$$", right: "$$", display: true },
      { left: "$", right: "$", display: false },
      { left: "\\(", right: "\\)", display: false },
      { left: "\\[", right: "\\]", display: true },
    ],
    macros: MACROS,
  });
  // Initialize syntax highlighting
  if (typeof hljs !== "undefined") {
    hljs.highlightAll();
  }
  const cardContent = document.querySelector(".card-content");
  if (cardContent) {
    cardContent.style.opacity = "1";
  }
});

document.addEventListener("keydown", function (event) {
  // Skip during text input.
  if (event.target.tagName === "INPUT" && event.target.type === "text") {
    return;
  }

  const keybindings = {
    " ": "reveal", // Space
    u: "undo",
    1: "forgot",
    2: "hard",
    3: "good",
    4: "easy",
  };

  if (keybindings[event.key]) {
    // Ignore modifiers.
    if (event.shiftKey || event.ctrlKey || event.altKey || event.metaKey) {
      return;
    }
    event.preventDefault();
    const id = keybindings[event.key];
    const node = document.getElementById(id);
    if (node) {
      node.click();
    }
  }
});
