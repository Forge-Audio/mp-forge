// The MPC Sample's own controls, as the program that runs an app on the device reads them, and its lights, as
// that program sets them.
//
// The pads, knobs, fader, dial and buttons speak MIDI on one port. What they send is read here as the things
// an app is told of, and what an app shows on its lights is written back as the bytes that set them. Which
// number is which control is not written here: MP Forge writes that into mp_forge_device_map.h.

#pragma once

#include "mp_forge_device_map.h"
#include "mpc_sample_app.h"

#include <array>
#include <cerrno>
#include <cmath>
#include <cstdint>
#include <unistd.h>
#include <vector>

namespace mp_forge_host {

inline bool writeAll(int to, const uint8_t* bytes, size_t count) {
    while (count > 0) {
        const ssize_t written = write(to, bytes, count);
        if (written < 0 && errno == EINTR) continue;
        if (written <= 0) return false;
        bytes += written;
        count -= static_cast<size_t>(written);
    }
    return true;
}

// One whole message of the controls: what kind it is and on which channel, and its two numbers.
struct Message {
    uint8_t status, first, second;

    uint8_t kind() const { return status & 0xF0; }
    int channel() const { return (status & 0x0F) + 1; }
};

// Puts the bytes of the controls together into messages, in whatever pieces the bytes arrive.
class MessageReader {
public:
    // Whether this byte ended a message, which is then in `message`.
    bool endsAMessage(uint8_t byte, Message& message) {
        if (byte >= kFirstByteSentInBetween) return false;
        if (byte >= kFirstStatusByte) return startAMessage(byte);
        if (isInsideALongMessage_ || status_ == kNoStatusYet) return false;
        return takeANumber(byte, message);
    }

private:
    static constexpr uint8_t kFirstStatusByte = 0x80, kStartsALongMessage = 0xF0, kFirstByteSentInBetween = 0xF8;
    static constexpr uint8_t kProgramChange = 0xC0, kChannelPressure = 0xD0;
    static constexpr uint8_t kNoStatusYet = 0;

    // A device may say the kind of message once for several messages in a row.
    bool startAMessage(uint8_t status) {
        isInsideALongMessage_ = status == kStartsALongMessage;
        status_ = status < kStartsALongMessage ? status : kNoStatusYet;
        numbersSoFar_ = 0;
        return false;
    }

    bool takeANumber(uint8_t number, Message& message) {
        numbers_[numbersSoFar_++] = number;
        if (numbersSoFar_ < numbersInAMessage()) return false;
        message = Message{status_, numbers_[0], numbersSoFar_ > 1 ? numbers_[1] : static_cast<uint8_t>(0)};
        numbersSoFar_ = 0;
        return true;
    }

    int numbersInAMessage() const {
        const uint8_t kind = status_ & 0xF0;
        return kind == kProgramChange || kind == kChannelPressure ? 1 : 2;
    }

    uint8_t status_ = kNoStatusYet;
    uint8_t numbers_[2] = {};
    int numbersSoFar_ = 0;
    bool isInsideALongMessage_ = false;
};

// Whoever is told what the musician did on the device.
struct HearsTheControls {
    virtual ~HearsTheControls() = default;
    virtual void touched(sample::Touched what, int32_t number, float value) = 0;
    // The main volume knob is the program's to follow: the device does not turn its sound down by itself.
    virtual void mainVolumeAt(float stands) = 0;
};

// Reads the bytes of the controls as what the musician did.
class Controls {
public:
    void read(const uint8_t* bytes, size_t count, HearsTheControls& heard) {
        Message message{};
        for (size_t at = 0; at < count; ++at)
            if (messages_.endsAMessage(bytes[at], message)) tellOf(message, heard);
    }

private:
    static constexpr uint8_t kNoteOff = 0x80, kNoteOn = 0x90, kController = 0xB0;
    static constexpr float kHardestHit = 127.0f, kFullyTurned = 127.0f;
    static constexpr int kNotAControl = -1;

    static void tellOf(const Message& message, HearsTheControls& heard) {
        if (message.channel() == device_map::kChannelOfThePads) return tellOfAPad(message, heard);
        if (message.channel() != device_map::kChannelOfTheOtherControls) return;
        if (message.kind() == kController) return tellOfSomethingTurned(message, heard);
        tellOfAButton(message, heard);
    }

    static bool isANote(const Message& message) { return message.kind() == kNoteOn || message.kind() == kNoteOff; }
    static bool isPressed(const Message& message) { return message.kind() == kNoteOn && message.second > 0; }

    template <size_t Count>
    static int placeOf(uint8_t note, const uint8_t (&notes)[Count]) {
        for (size_t place = 0; place < Count; ++place)
            if (notes[place] == note) return static_cast<int>(place);
        return kNotAControl;
    }

    // How hard a pad is pressed while it is held is left out: the kit tells an app of the hit and the letting go.
    static void tellOfAPad(const Message& message, HearsTheControls& heard) {
        const int pad = placeOf(message.first, device_map::kNotesOfThePads);
        if (pad == kNotAControl || !isANote(message)) return;
        heard.touched(sample::Touched::Pad, pad + 1, isPressed(message) ? static_cast<float>(message.second) / kHardestHit : 0.0f);
    }

    static void tellOfAButton(const Message& message, HearsTheControls& heard) {
        const int button = placeOf(message.first, device_map::kNotesOfTheButtons);
        if (button == kNotAControl || !isANote(message)) return;
        heard.touched(sample::Touched::Button, button, isPressed(message) ? 1.0f : 0.0f);
    }

    static void tellOfSomethingTurned(const Message& message, HearsTheControls& heard) {
        const uint8_t controller = message.first;
        const float stands = static_cast<float>(message.second) / kFullyTurned;
        if (controller >= device_map::kFirstKnob && controller <= device_map::kLastKnob)
            return heard.touched(sample::Touched::Knob, controller - device_map::kFirstKnob + 1, stands);
        if (controller == device_map::kFader) return heard.touched(sample::Touched::Fader, 0, stands);
        if (controller == device_map::kMainVolume) return heard.mainVolumeAt(stands);
        if (controller == device_map::kDial && message.second != 0) heard.touched(sample::Touched::Encoder, clicksTurned(message.second), 0.0f);
    }

    // The dial says how many clicks it was turned: to the right as they are, to the left counted down from 128.
    static int clicksTurned(uint8_t value) { return value <= device_map::kMostClicksToTheRight ? value : value - 128; }

    MessageReader messages_;
};

// The lights of the device, set through the port its controls speak on. Only a light that changed is sent.
class DeviceLights {
public:
    static constexpr size_t kPadBytes = static_cast<size_t>(sample::kPads) * 3;

    explicit DeviceLights(int controls) : controls_(controls) {}

    using PadColours = std::array<uint8_t, kPadBytes>;
    using LitButtons = std::array<uint8_t, sample::kButtons>;
    using LevelLights = std::array<uint8_t, sample::kLevelStrips>;

    // Only the lights that changed are set, and all of them while nothing is known of how they stand.
    void show(const PadColours& padColours, const LitButtons& litButtons, const LevelLights& levelLights) {
        std::vector<uint8_t> sent;
        addPadsThatChanged(padColours, sent);
        addButtonsThatChanged(litButtons, sent);
        addStripsThatChanged(levelLights, sent);
        writeAll(controls_, sent.data(), sent.size());
        padColours_ = padColours;
        litButtons_ = litButtons;
        levelLights_ = levelLights;
        isKnown_ = true;
    }

    void goDark() {
        isKnown_ = false;
        show({}, {}, {});
    }

private:
    static constexpr uint8_t kSetsAButtonLight = 0xB0;
    static constexpr uint8_t kSetsALevelStrip = 0xB0;

    void addPadsThatChanged(const PadColours& padColours, std::vector<uint8_t>& sent) const {
        for (int pad = 0; pad < sample::kPads; ++pad)
            if (!isKnown_ || !isTheSameColour(padColours, pad)) addColourOf(pad, &padColours[static_cast<size_t>(pad) * 3], sent);
    }

    void addButtonsThatChanged(const LitButtons& litButtons, std::vector<uint8_t>& sent) const {
        for (size_t button = 0; button < litButtons.size(); ++button)
            if (!isKnown_ || litButtons[button] != litButtons_[button]) addLightOf(static_cast<int>(button), litButtons[button] != 0, sent);
    }

    void addStripsThatChanged(const LevelLights& levelLights, std::vector<uint8_t>& sent) const {
        for (size_t strip = 0; strip < levelLights.size(); ++strip)
            if (!isKnown_ || levelLights[strip] != levelLights_[strip]) addLevelOf(strip, levelLights[strip], sent);
    }

    // A strip is sent one bit for each of its lights that is lit, the bottom light the lowest bit.
    static void addLevelOf(size_t strip, uint8_t lit, std::vector<uint8_t>& sent) {
        const int litFromTheBottom = (1 << std::min<int>(lit, sample::kLightsAStrip)) - 1;
        sent.insert(sent.end(), {kSetsALevelStrip, device_map::kLevelStrips[strip], static_cast<uint8_t>(litFromTheBottom)});
    }
    static constexpr float kBrightestAnAppSays = 255.0f, kBrightestAPadTakes = 127.0f;

    bool isTheSameColour(const std::array<uint8_t, kPadBytes>& padColours, int pad) const {
        const size_t at = static_cast<size_t>(pad) * 3;
        return std::equal(&padColours[at], &padColours[at] + 3, &padColours_[at]);
    }

    static void addColourOf(int pad, const uint8_t* colour, std::vector<uint8_t>& sent) {
        sent.insert(sent.end(), std::begin(device_map::kStartsAPadColour), std::end(device_map::kStartsAPadColour));
        sent.push_back(static_cast<uint8_t>(pad));
        const uint8_t strongest = std::max({colour[0], colour[1], colour[2]});
        for (int part = 0; part < 3; ++part) sent.push_back(asAPadLightsIt(colour[part], strongest));
        sent.push_back(device_map::kEndsAPadColour);
    }

    // An app says a colour as a screen shows it, where a part at half its number gives far less than half the
    // light. A pad's light is in step with its number. So each part is weighed against the strongest as a screen
    // weighs it, which keeps the colour, and the strongest is sent in step with its own number.
    static uint8_t asAPadLightsIt(uint8_t part, uint8_t strongest) {
        if (strongest == 0) return 0;
        const float brightness = static_cast<float>(strongest) / kBrightestAnAppSays * kBrightestAPadTakes;
        const float weighed = std::pow(static_cast<float>(part) / static_cast<float>(strongest), device_map::kFallOfAScreensLight);
        return static_cast<uint8_t>(std::lround(weighed * brightness));
    }

    static void addLightOf(int button, bool isLit, std::vector<uint8_t>& sent) {
        if (!device_map::kButtonHasALight[button]) return;
        sent.insert(sent.end(), {kSetsAButtonLight, device_map::kNotesOfTheButtons[button], isLit ? device_map::kLightOn : device_map::kLightOff});
    }

    int controls_;
    bool isKnown_ = false;
    PadColours padColours_{};
    LitButtons litButtons_{};
    LevelLights levelLights_{};
};

}  // namespace mp_forge_host
