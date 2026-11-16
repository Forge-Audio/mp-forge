// The plugin architect's questions, shown in the conversation before a new plugin is built.
//
// The questions are shown one at a time. Each comes with choices to press. The musician can also answer in their
// own words, leave a question unanswered, ask for more questions after the last one, or go back and change an
// answer. Whatever they answer is added to their description.

(function () {
  "use strict";

  const { element } = window.helpers;
  const { button } = window.sheets;

  const OWN_WORDS = "Or say it your own way";
  const LONGEST_OWN_WORDS = 200;

  const MOVES_ON_AFTER_MS = 220;

  // Pressing a choice picks it. Where only one can be picked, it takes the place of the one picked before, and
  // `whenPicked` is told, so the next question can be shown.
  function choiceButton(words, question, row, whenPicked) {
    const made = button("option-button architect-choice-button", words);
    made.setAttribute("aria-pressed", "false");
    made.addEventListener("click", () => {
      const wasPicked = made.getAttribute("aria-pressed") === "true";
      if (!question.pickSeveral) row.querySelectorAll("button").forEach((other) => other.setAttribute("aria-pressed", "false"));
      made.setAttribute("aria-pressed", String(!wasPicked));
      if (!question.pickSeveral && !wasPicked) whenPicked();
    });
    return made;
  }

  function ownWordsBox(question, whenEntered) {
    const box = element("input", "item-input architect-own-words");
    box.type = "text";
    box.maxLength = LONGEST_OWN_WORDS;
    box.placeholder = OWN_WORDS;
    box.setAttribute("aria-label", `${question.asks} ${OWN_WORDS}`);
    box.addEventListener("keydown", (event) => { if (event.key === "Enter") whenEntered(); });
    return box;
  }

  // One question with its choices, and how to read what was answered. Nothing answered reads as nothing.
  // `whenAnswered` is told once the question has its one answer, by a press or by Enter in the box.
  function questionBlock(question, whenAnswered) {
    const block = element("div", "architect-question");
    const row = element("div", "options");
    const ownWords = ownWordsBox(question, whenAnswered);
    row.setAttribute("role", "group");
    row.setAttribute("aria-label", question.pickSeveral ? "Pick any that fit" : "Pick one");
    row.append(...question.choices.map((words) => choiceButton(words, question, row, whenAnswered)));
    block.append(element("div", "architect-asks", question.asks), element("div", "soft small", question.pickSeveral ? "Pick any that fit." : "Pick one."), row, ownWords);
    const answer = () => {
      const picked = Array.from(row.querySelectorAll('button[aria-pressed="true"]'), (made) => made.textContent);
      return [...picked, ownWords.value.trim()].filter(Boolean).join(", ");
    };
    return { block, answer: () => ({ question: question.asks, answer: answer() }) };
  }

  function cardButton(className, words, onPressed, says) {
    const made = button(className, words);
    made.addEventListener("click", onPressed);
    if (says) made.title = says;
    return made;
  }

  // The architect's questions as one card that shows them one at a time. BACK and NEXT step through them, and a
  // question with one answer moves on by itself once it has it. On the last question the card offers
  // `listeners.onBuild` and `onMore`; `onStartOver` is there throughout.
  // `answers()` gives every question that was answered, with its answer.
  function card(questions, listeners) {
    const made = element("div", "message architect");
    const whichOne = element("div", "soft small architect-which");
    let shown = 0;
    const stepBy = (questionsOn) => show(shown + questionsOn);
    const blocks = questions.map((question) => questionBlock(question, () => setTimeout(() => stepBy(1), MOVES_ON_AFTER_MS)));
    const back = cardButton("item-button", "BACK", () => stepBy(-1));
    const next = cardButton("item-button done", "NEXT", () => stepBy(1), "Leave this one unanswered if you like. Anything unanswered is left to the AI.");
    const build = cardButton("item-button done", "BUILD IT WITH THESE", listeners.onBuild);
    const more = cardButton("item-button", "ASK ME MORE", listeners.onMore, "Keeps these answers and asks the architect for another round of questions.");
    const startOver = cardButton("item-button", "START OVER", listeners.onStartOver, "Lets go of this idea and these answers. Nothing has been built.");

    function show(question) {
      shown = Math.max(0, Math.min(question, blocks.length - 1));
      const isLast = shown === blocks.length - 1;
      blocks.forEach((made, position) => { made.block.hidden = position !== shown; });
      whichOne.textContent = `Question ${shown + 1} of ${blocks.length}`;
      back.disabled = shown === 0;
      next.hidden = isLast;
      build.hidden = !isLast;
      more.hidden = !isLast;
    }

    const buttons = element("div", "architect-buttons");
    buttons.append(back, next, build, more, startOver);
    made.append(element("div", "field-label", "Plugin architect"), whichOne, ...blocks.map((made) => made.block), buttons);
    show(0);
    return { made, answers: () => blocks.map((block) => block.answer()).filter((given) => given.answer) };
  }

  // What was answered, one line each, as the musician's own next message.
  function wordsOf(answers) {
    return answers.map((given) => `${given.question} ${given.answer}`).join("\n");
  }

  // The description the plugin is built from: the first idea, what was chosen when asked, and anything added last.
  function describedWith(idea, answers, lastWords) {
    const chosen = answers.length ? `\n\nWhat I chose when you asked:\n${answers.map((given) => `- ${given.question} ${given.answer}`).join("\n")}` : "";
    const added = lastWords ? `\n\nAlso: ${lastWords}` : "";
    return `${idea}${chosen}${added}`;
  }

  window.pluginArchitect = { card, wordsOf, describedWith };
})();
