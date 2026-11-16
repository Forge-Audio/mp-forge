// How long a piece of work took, measured on this computer's own clock and said in plain words.
(function () {
  const MS_IN_A_SECOND = 1000;
  const SECONDS_IN_A_MINUTE = 60;
  const MINUTES_IN_AN_HOUR = 60;

  // The moment a piece of work begins, to be handed to `secondsSince` when it ends.
  function now() {
    return performance.now();
  }

  function secondsSince(began) {
    return Math.round((now() - began) / MS_IN_A_SECOND);
  }

  // The two largest measures that say it: seconds alone, minutes with seconds, or hours with minutes.
  // A smaller measure that comes to nothing is left out.
  function inWords(seconds) {
    const minutes = Math.floor(seconds / SECONDS_IN_A_MINUTE);
    const hours = Math.floor(minutes / MINUTES_IN_AN_HOUR);
    const measures = hours
      ? [[hours, "h"], [minutes % MINUTES_IN_AN_HOUR, "min"]]
      : [[minutes, "min"], [seconds % SECONDS_IN_A_MINUTE, "s"]];
    const said = measures.filter(([amount]) => amount > 0).map(([amount, measure]) => `${amount} ${measure}`);
    return said.length ? said.join(" ") : "under a second";
  }

  window.timeTaken = { now, secondsSince, inWords };
})();
