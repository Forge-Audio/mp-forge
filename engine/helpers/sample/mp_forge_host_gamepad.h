// A game controller plugged into the MPC Sample's USB socket, read for the app.
//
// The device's one USB socket usually faces a computer, and its system then lists nowhere for a controller to
// be plugged in. mp_forge_host_usb_socket.h turns the socket round for an app that takes a controller.
//
// Two things are kept apart here. What an app is told of a controller is the same however the controller is
// read: a button by where it sits, a stick from -1 to 1 with up as more, a trigger from 0 to 1, and only what
// changed. How a controller is read differs. This file reads the ones the device's own system lists, as files
// of events in /dev/input. The system has no driver for some families of controller, which are read straight
// from USB instead: see mp_forge_host_xbox_gamepad.h.
//
// Controllers the system lists do not agree on which number is which button. This follows what the system
// says each number means, which is right for most. A controller that numbers its buttons its own way will
// have some swapped.

#pragma once

#include "mpc_sample_app.h"

#include <array>
#include <cerrno>
#include <chrono>
#include <cmath>
#include <cstdio>
#include <fcntl.h>
#include <unistd.h>

#if defined(__linux__)
#include <linux/input.h>
#include <sys/ioctl.h>
#endif

namespace mp_forge_host {

struct HearsTheGamepad {
    virtual ~HearsTheGamepad() = default;
    virtual void gamepadDid(sample::Touched what, int32_t number, float value) = 0;
};

// What has been said of a controller so far, so that an app is told only of what changes.
class SaidOfAGamepad {
public:
    void button(sample::GamepadButton button, bool isPressed, HearsTheGamepad& heard) {
        bool& isHeld = held_[static_cast<size_t>(button)];
        if (isHeld == isPressed) return;
        isHeld = isPressed;
        heard.gamepadDid(sample::Touched::GamepadButton, static_cast<int32_t>(button), isPressed ? 1.0f : 0.0f);
    }

    // Where a stick stands along one of its directions, from -1 to 1. A stick at rest is never quite still,
    // so the middle counts as rest.
    void stick(sample::GamepadAxis along, float stands, HearsTheGamepad& heard) {
        moved(along, std::fabs(stands) < kCountsAsRest ? 0.0f : stands, heard);
    }

    // How far a trigger is pulled, from 0 to 1.
    void trigger(sample::GamepadAxis trigger, float pulled, HearsTheGamepad& heard) { moved(trigger, pulled, heard); }

    // For a controller that does not say its triggers as buttons: the button is said for it, once pulled halfway.
    void triggerWithItsButton(sample::GamepadAxis pulledTrigger, float pulled, HearsTheGamepad& heard) {
        trigger(pulledTrigger, pulled, heard);
        const auto asAButton = pulledTrigger == sample::GamepadAxis::LeftTrigger ? sample::GamepadButton::LeftTrigger : sample::GamepadButton::RightTrigger;
        button(asAButton, pulled >= kPulledFarEnoughToBePressed, heard);
    }

    // The app is told that everything held on the controller was let go, so nothing is left held for good.
    void everythingLetGo(HearsTheGamepad& heard) {
        for (int held = 0; held < sample::kGamepadButtons; ++held) button(static_cast<sample::GamepadButton>(held), false, heard);
        for (int along = 0; along < sample::kGamepadAxes; ++along) moved(static_cast<sample::GamepadAxis>(along), 0.0f, heard);
    }

private:
    static constexpr float kCountsAsRest = 0.12f;
    // A stick or a trigger is said again only once it has moved this far, so that a hand resting on it does
    // not fill the app's ears.
    static constexpr float kSmallestMoveWorthSaying = 0.01f;
    static constexpr float kPulledFarEnoughToBePressed = 0.5f;

    void moved(sample::GamepadAxis along, float stands, HearsTheGamepad& heard) {
        float& lastSaid = stands_[static_cast<size_t>(along)];
        const bool isWorthSaying = std::fabs(stands - lastSaid) >= kSmallestMoveWorthSaying || (stands == 0.0f && lastSaid != 0.0f);
        if (!isWorthSaying) return;
        lastSaid = stands;
        heard.gamepadDid(sample::Touched::GamepadAxis, static_cast<int32_t>(along), stands);
    }

    std::array<bool, sample::kGamepadButtons> held_{};
    std::array<float, sample::kGamepadAxes> stands_{};
};

#if defined(__linux__)

// A controller the device's own system lists among what is plugged in.
class ListedGamepad {
public:
    // What to wait on for news of the controller. Nothing, as a wait knows it, while none is plugged in.
    int file() const { return file_; }

    // Looks for a controller while none is held, once a second at most.
    void lookForOne() {
        if (file_ >= 0 || Clock::now() < looksAgainAt_) return;
        looksAgainAt_ = Clock::now() + kLooksAgainEvery;
        for (int number = 0; number < kMostThingsListed && file_ < 0; ++number) takeIfItIsOne(number);
        sayOnceWhenNoneWasFound();
    }

    // Tells what the controller has done since it was last heard. One that has gone is let go of.
    void hear(HearsTheGamepad& heard) {
        input_event done;
        ssize_t count = 0;
        while ((count = read(file_, &done, sizeof done)) == static_cast<ssize_t>(sizeof done)) tellOf(done, heard);
        const bool hasNoMoreToSay = count < 0 && (errno == EAGAIN || errno == EINTR);
        if (!hasNoMoreToSay) letGoOfIt(heard);
    }

private:
    using Clock = std::chrono::steady_clock;

    static constexpr auto kLooksAgainEvery = std::chrono::seconds(1);
    static constexpr int kMostThingsListed = 16;
    static constexpr int kNoButton = -1;

    // Where the system counts from, and the range a direction of this controller runs over.
    struct Range {
        int least = 0, most = 0;
        bool isThere() const { return most > least; }
        float shareOf(int value) const { return static_cast<float>(value - least) / static_cast<float>(most - least); }
    };

    void takeIfItIsOne(int number) {
        char place[32];
        std::snprintf(place, sizeof place, "/dev/input/event%d", number);
        const int found = open(place, O_RDONLY | O_NONBLOCK | O_CLOEXEC);
        if (found < 0) return;
        hasSeenSomethingListed_ = true;
        if (!hasAGameControllersButtons(found)) { close(found); return; }
        file_ = found;
        learnItsRanges();
        sayItWasFound();
    }

    static bool has(const unsigned long* bits, int number) {
        const int bitsAWord = static_cast<int>(sizeof(unsigned long) * 8);
        return (bits[number / bitsAWord] >> (number % bitsAWord) & 1UL) != 0;
    }

    // A keyboard or a mouse has buttons too, but none of a game controller's.
    bool hasAGameControllersButtons(int found) {
        unsigned long keys[KEY_MAX / (sizeof(unsigned long) * 8) + 1] = {};
        if (ioctl(found, EVIOCGBIT(EV_KEY, sizeof keys), keys) < 0) return false;
        tellsItsTriggersAsButtons_ = has(keys, BTN_TL2);
        return has(keys, BTN_SOUTH) || has(keys, BTN_JOYSTICK);
    }

    void learnItsRanges() {
        for (const int along : {ABS_X, ABS_Y, ABS_Z, ABS_RX, ABS_RY, ABS_RZ}) ranges_[static_cast<size_t>(along)] = rangeOf(along);
        // Most controllers have their right stick on two directions of its own, and the triggers on the two
        // left over. One without those has its right stick where the triggers would be.
        hasItsRightStickOnItsOwn_ = ranges_[ABS_RX].isThere() && ranges_[ABS_RY].isThere();
    }

    Range rangeOf(int along) const {
        input_absinfo said{};
        if (ioctl(file_, EVIOCGABS(along), &said) < 0) return Range{};
        return Range{said.minimum, said.maximum};
    }

    void sayItWasFound() const {
        char name[128] = "one with no name";
        ioctl(file_, EVIOCGNAME(sizeof name), name);
        std::fprintf(stderr, "A game controller was found: %s\n", name);
    }

    void sayOnceWhenNoneWasFound() {
        if (file_ >= 0 || hasSaidNoneWasFound_) return;
        hasSaidNoneWasFound_ = true;
        std::fprintf(stderr, hasSeenSomethingListed_ ? "The device's system lists no game controller among what is plugged in.\n"
                                                     : "The device's system lists nothing that is plugged in at /dev/input.\n");
    }

    void tellOf(const input_event& done, HearsTheGamepad& heard) {
        if (done.type == EV_KEY) tellOfAButton(done.code, done.value != 0, heard);
        if (done.type == EV_ABS) tellOfADirection(done.code, done.value, heard);
    }

    void tellOfAButton(int code, bool isPressed, HearsTheGamepad& heard) {
        const int button = buttonOf(code);
        if (button != kNoButton) said_.button(static_cast<sample::GamepadButton>(button), isPressed, heard);
    }

    // The buttons as the system numbers them for a game controller, and, for a controller that only counts
    // its buttons from one, the same order the kit has them in.
    static int buttonOf(int code) {
        using Button = sample::GamepadButton;
        switch (code) {
            case BTN_SOUTH: return static_cast<int>(Button::South);
            case BTN_EAST: return static_cast<int>(Button::East);
            case BTN_WEST: return static_cast<int>(Button::West);
            case BTN_NORTH: return static_cast<int>(Button::North);
            case BTN_TL: return static_cast<int>(Button::LeftShoulder);
            case BTN_TR: return static_cast<int>(Button::RightShoulder);
            case BTN_TL2: return static_cast<int>(Button::LeftTrigger);
            case BTN_TR2: return static_cast<int>(Button::RightTrigger);
            case BTN_SELECT: return static_cast<int>(Button::Select);
            case BTN_START: return static_cast<int>(Button::Start);
            case BTN_THUMBL: return static_cast<int>(Button::LeftStickIn);
            case BTN_THUMBR: return static_cast<int>(Button::RightStickIn);
            case BTN_DPAD_UP: return static_cast<int>(Button::Up);
            case BTN_DPAD_DOWN: return static_cast<int>(Button::Down);
            case BTN_DPAD_LEFT: return static_cast<int>(Button::Left);
            case BTN_DPAD_RIGHT: return static_cast<int>(Button::Right);
            case BTN_MODE: return static_cast<int>(Button::Home);
            default: return buttonCountedFromOne(code);
        }
    }

    static int buttonCountedFromOne(int code) {
        const int place = code - BTN_JOYSTICK;
        const bool isOneTheKitHas = place >= 0 && place <= static_cast<int>(sample::GamepadButton::RightStickIn);
        return isOneTheKitHas ? place : kNoButton;
    }

    void tellOfADirection(int code, int value, HearsTheGamepad& heard) {
        using Axis = sample::GamepadAxis;
        using Button = sample::GamepadButton;
        switch (code) {
            case ABS_X: return said_.stick(Axis::LeftX, acrossAStick(code, value), heard);
            case ABS_Y: return said_.stick(Axis::LeftY, -acrossAStick(code, value), heard);
            case ABS_RX: return said_.stick(Axis::RightX, acrossAStick(code, value), heard);
            case ABS_RY: return said_.stick(Axis::RightY, -acrossAStick(code, value), heard);
            case ABS_Z: return tellOfWhatSharesWithATrigger(Axis::LeftTrigger, Axis::RightX, code, value, heard);
            case ABS_RZ: return tellOfWhatSharesWithATrigger(Axis::RightTrigger, Axis::RightY, code, value, heard);
            case ABS_HAT0X: return tellOfTheCross(Button::Left, Button::Right, value, heard);
            case ABS_HAT0Y: return tellOfTheCross(Button::Up, Button::Down, value, heard);
            default: return;
        }
    }

    void tellOfWhatSharesWithATrigger(sample::GamepadAxis trigger, sample::GamepadAxis ofTheRightStick, int code, int value, HearsTheGamepad& heard) {
        if (hasItsRightStickOnItsOwn_) return tellOfATrigger(trigger, ranges_[static_cast<size_t>(code)].shareOf(value), heard);
        const float stands = acrossAStick(code, value);
        said_.stick(ofTheRightStick, ofTheRightStick == sample::GamepadAxis::RightY ? -stands : stands, heard);
    }

    void tellOfATrigger(sample::GamepadAxis trigger, float pulled, HearsTheGamepad& heard) {
        if (tellsItsTriggersAsButtons_) said_.trigger(trigger, pulled, heard);
        else said_.triggerWithItsButton(trigger, pulled, heard);
    }

    // From -1 to 1. A direction the controller has not got stays at rest.
    float acrossAStick(int code, int value) const {
        const Range range = ranges_[static_cast<size_t>(code)];
        return range.isThere() ? range.shareOf(value) * 2.0f - 1.0f : 0.0f;
    }

    // The cross on the left of many controllers is said as a direction that is one way, the other, or neither.
    void tellOfTheCross(sample::GamepadButton whenLess, sample::GamepadButton whenMore, int value, HearsTheGamepad& heard) {
        said_.button(whenLess, value < 0, heard);
        said_.button(whenMore, value > 0, heard);
    }

    void letGoOfIt(HearsTheGamepad& heard) {
        close(file_);
        file_ = -1;
        said_.everythingLetGo(heard);
        std::fprintf(stderr, "The game controller was unplugged.\n");
    }

    int file_ = -1;
    Clock::time_point looksAgainAt_{};
    bool hasSeenSomethingListed_ = false;
    bool hasSaidNoneWasFound_ = false;
    bool tellsItsTriggersAsButtons_ = false;
    bool hasItsRightStickOnItsOwn_ = false;
    std::array<Range, ABS_RZ + 1> ranges_{};
    SaidOfAGamepad said_;
};

#else

// Only the device lists what is plugged into it this way. Anywhere else there is no controller to read here.
class ListedGamepad {
public:
    int file() const { return -1; }
    void lookForOne() {}
    void hear(HearsTheGamepad& /*heard*/) {}
};

#endif

}  // namespace mp_forge_host
