// Keeps a scrolling list at its newest words, the way a chat does.
//
// New words glide into view for as long as the reader is at the end. Once they scroll up to read something
// older the list stays where they put it, until they come back down or say something themselves.
//
// The glide is stepped here, a frame at a time, towards wherever the end is at that moment. The browser's own
// smooth scrolling aims at where the end was when it began, and in the app's window (WebKit) it was left
// short of the end when the list changed size or was moved on the way: the newest words then sat out of
// sight until something else scrolled the list.

(function () {
  "use strict";

  const NEAR_THE_END_PX = 48;
  const AT_THE_END_PX = 1;
  // How long a glide takes to cover about two thirds of what is left of the way.
  const GLIDE_EASES_OVER_MS = 90;

  function endFollower(list) {
    let following = true;
    let gliding = true;
    let lastPlace = 0;
    let nextStep = null;
    let lastStepAt = 0;

    function isNearTheEnd() {
      return leftToTheEnd() < NEAR_THE_END_PX;
    }

    // Gliding down never moves the list up, so only the reader's own scrolling can stop the following.
    function noteWhereTheReaderIs() {
      const movedUp = list.scrollTop < lastPlace;
      if (isNearTheEnd()) following = true;
      else if (movedUp) following = false;
      lastPlace = list.scrollTop;
    }

    function leftToTheEnd() {
      return list.scrollHeight - list.clientHeight - list.scrollTop;
    }

    // The share of what is left that one frame covers. It goes by the time since the last frame, so the glide
    // takes as long whatever the screen's rate, and a window that is drawn only now and then still arrives.
    function shareCoveredSince(lastStep, now) {
      return 1 - Math.exp(-Math.max(0, now - lastStep) / GLIDE_EASES_OVER_MS);
    }

    function stepTowardsTheEnd(now) {
      nextStep = null;
      const left = leftToTheEnd();
      if (!following || left < AT_THE_END_PX) return;
      list.scrollTop += Math.max(1, Math.ceil(left * shareCoveredSince(lastStepAt, now)));
      lastPlace = list.scrollTop;
      lastStepAt = now;
      nextStep = requestAnimationFrame(stepTowardsTheEnd);
    }

    function stopGliding() {
      if (nextStep !== null) cancelAnimationFrame(nextStep);
      nextStep = null;
    }

    function restAtTheEnd() {
      stopGliding();
      list.scrollTop = list.scrollHeight;
      lastPlace = list.scrollTop;
    }

    function glide() {
      if (!following || !gliding) return;
      if (window.motion.keepsStill()) return restAtTheEnd();
      if (nextStep !== null) return;
      lastStepAt = performance.now();
      nextStep = requestAnimationFrame(stepTowardsTheEnd);
    }

    // The app's window (WebKit) can leave a list it has just been moved to the end of unpainted, blank until it
    // is next scrolled. So once the new words are laid out the list is scrolled for it: a hair short of the end
    // for one frame, then to the end.
    function paintAtTheEnd() {
      if (!following) return;
      list.scrollTop = Math.max(0, list.scrollHeight - list.clientHeight - 1);
      requestAnimationFrame(() => following && restAtTheEnd());
    }

    function jump() {
      following = true;
      restAtTheEnd();
      requestAnimationFrame(paintAtTheEnd);
    }

    // Makes a change that puts other words in the list, as opening another conversation does, and comes to rest
    // at their end at once. A glide still under way is stopped first and none is begun while the words change:
    // a glide across a whole long conversation is no use, and it is what a jump must never meet half way.
    function arriveWith(change) {
      stopGliding();
      gliding = false;
      change();
      gliding = true;
      jump();
    }

    function followAgain() {
      following = true;
    }

    // Something beside the list is about to change size, which changes how much of the list shows. A list that
    // is following comes to rest at its end. One being read further up keeps the same words at its bottom edge.
    function keepPlaceWhile(change) {
      const hiddenBelow = leftToTheEnd();
      change();
      if (following) return restAtTheEnd();
      list.scrollTop = list.scrollHeight - list.clientHeight - hiddenBelow;
      lastPlace = list.scrollTop;
    }

    // The list changing size, as when the conversation is made narrower, lays its words out again. A list
    // that is following is kept at its end the whole time it does.
    function stayAtTheEndWhileFollowing() {
      if (following) restAtTheEnd();
    }

    list.addEventListener("scroll", noteWhereTheReaderIs, { passive: true });
    new ResizeObserver(stayAtTheEndWhileFollowing).observe(list);
    return { glide, jump, arriveWith, followAgain, keepPlaceWhile };
  }

  window.endFollower = endFollower;
})();
