// Copyright 2025–2026 Fernando Borretti
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
  // Render inline math
  document.querySelectorAll(".math-inline").forEach(function (element) {
    katex.render(element.textContent, element, {
      displayMode: false,
      throwOnError: false,
      macros: MACROS,
    });
  });
  // Render display math
  document.querySelectorAll(".math-display").forEach(function (element) {
    katex.render(element.textContent, element, {
      displayMode: true,
      throwOnError: false,
      macros: MACROS,
    });
  });
  // Initialize syntax highlighting
  if (typeof hljs !== "undefined") {
    hljs.highlightAll();
  }
  // Make the card content visible, after we've rendered the math and
  // highlighted the code. Unlike the drill interface, a browse page can show
  // many cards at once, so reveal all of them.
  document.querySelectorAll(".card-content").forEach(function (element) {
    element.style.opacity = "1";
  });
});
