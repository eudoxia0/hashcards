document.addEventListener("DOMContentLoaded", function () {
  renderMathInElement(document.body, {
    delimiters: [
      { left: "$", right: "$", display: false },
      { left: "$$", right: "$$", display: true },
      { left: "\\(", right: "\\)", display: false },
      { left: "\\[", right: "\\]", display: true },
    ],
    macros: {
      "\\foo": "bar",
    },
  });
});

document.addEventListener("keydown", function (event) {
  // Skip during text input.
  if (event.target.tagName === "INPUT" && event.target.type === "text") {
    return;
  }

  const keybindings = {
    " ": "reveal", // Space
    1: "forgot",
    2: "hard",
    3: "good",
    4: "easy",
  };

  if (keybindings[event.key]) {
    event.preventDefault();
    const id = keybindings[event.key];
    const node = document.getElementById(id);
    if (node) {
      node.click();
    }
  }
});
