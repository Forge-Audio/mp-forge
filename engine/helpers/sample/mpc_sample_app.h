// MP Forge's kit for an app that runs on the MPC Sample.
//
// An app is one class that says what happens when the musician touches the device, what sound comes out, and
// what the screen shows. MP Forge runs the same app on the drawing of the MPC Sample on a computer and on the
// device itself, so what is seen and heard on the one is what the other does.
//
// MP Forge writes this file into every Sample app before it is built. Do not change it: it is written again.
//
// Never two at once: MP Forge calls one of an app's functions at a time, so an app needs no locks. `makeSound`
// is called very often and must come back at once: no waiting, no files, no memory asked for there.
//
// On the device, holding SHIFT and STOP together for a second leaves the app. MP Forge sees to that itself,
// whatever the app does: an app is told of both buttons like any other, and gives that hold no use of its own.
//
// An app never opens a file. The sound files the musician added to it are read by MP Forge and handed over
// whole, as `samples()`. A file on the SD card is asked for through `card()`, or picked by the musician on the
// kit's own page, `browseCard`, and MP Forge loads it and hands it over the same way.
//
// The same goes for whatever is plugged in. MP Forge reads a MIDI keyboard or a game controller and tells the
// app what was done on it: see `noteOn` and `gamepadPressed`.

#pragma once

#include <algorithm>
#include <array>
#include <cctype>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>

namespace sample {

constexpr int kScreenWidth = 320;
constexpr int kScreenHeight = 240;
constexpr int kPads = 16;
constexpr int kKnobs = 3;
constexpr int kMostSamples = 64;
constexpr int kLongestSampleName = 64;
constexpr int kMostCardEntries = 200;
constexpr int kLongestCardPath = 255;

struct Colour {
    uint8_t red = 0, green = 0, blue = 0;
};

// The buttons of the device, by the words printed on them. `Encoder` is the big dial being pressed in.
enum class Button {
    B1, B2, B3, Sample, Seq, PadFx, KnobFx, Shift, PadBank, Erase, NoteRepeat, Chop, Mute, Loop, SixteenLevels,
    SampleSelect, TapTempo, Minus, Plus, SampleRecord, SeqRecord, Stop, Play, Encoder,
    Count
};

constexpr int kButtons = static_cast<int>(Button::Count);

// The buttons of a game controller plugged in, by where they sit, since makers print different letters on them.
// `South` is the lowest of the four on the right: A on some controllers, a cross on others. `Up`, `Down`, `Left`
// and `Right` are the cross on the left. A trigger is a button too, pressed once it is pulled halfway, and so is
// a stick pushed in.
enum class GamepadButton : int32_t {
    South, East, West, North, LeftShoulder, RightShoulder, LeftTrigger, RightTrigger, Select, Start, LeftStickIn, RightStickIn,
    Up, Down, Left, Right, Home,
    Count
};

constexpr int kGamepadButtons = static_cast<int>(GamepadButton::Count);

// A stick or a trigger of a game controller, by the side it is on.
enum class GamepadSide : int32_t { Left = 0, Right = 1 };

// For MP Forge, which says where a stick stands one direction at a time, and each trigger by itself.
enum class GamepadAxis : int32_t { LeftX = 0, LeftY = 1, RightX = 2, RightY = 3, LeftTrigger = 4, RightTrigger = 5, Count };

constexpr int kGamepadAxes = static_cast<int>(GamepadAxis::Count);

// The screen, 320 dots across and 240 down, with (0, 0) at the top left. Whatever is drawn outside it is left out.
class Screen {
public:
    void clear(Colour colour) { fill(0, 0, kScreenWidth, kScreenHeight, colour); }

    void dot(int x, int y, Colour colour) {
        if (x < 0 || y < 0 || x >= kScreenWidth || y >= kScreenHeight) return;
        uint8_t* at = &bytes_[static_cast<size_t>(y * kScreenWidth + x) * kBytesADot];
        at[0] = colour.red; at[1] = colour.green; at[2] = colour.blue; at[3] = 255;
    }

    void fill(int x, int y, int width, int height, Colour colour) {
        for (int row = std::max(0, y); row < std::min(kScreenHeight, y + height); ++row)
            for (int column = std::max(0, x); column < std::min(kScreenWidth, x + width); ++column)
                dot(column, row, colour);
    }

    // Writes words in the kit's own letters: capitals, digits and - + : . / _ Each letter is 3 dots wide and 5 tall,
    // made `size` times bigger. Small letters are written as capitals, and anything else is left as a gap.
    void write(const char* words, int x, int y, Colour colour, int size = 1) {
        for (int place = 0; words[place] != '\0'; ++place)
            letter(words[place], x + place * widthOfALetter(size), y, colour, size);
    }

    static int widthOfALetter(int size = 1) { return (kDotsAcrossALetter + 1) * size; }
    static int heightOfALetter(int size = 1) { return kRowsInALetter * size; }
    static int widthOf(const char* words, int size = 1) { return static_cast<int>(std::strlen(words)) * widthOfALetter(size); }

    // The picture as MP Forge takes it: four bytes a dot (red, green, blue, 255), row after row.
    const uint8_t* bytes() const { return bytes_.data(); }
    static constexpr size_t kByteCount = static_cast<size_t>(kScreenWidth) * kScreenHeight * 4;

private:
    static constexpr int kBytesADot = 4;
    static constexpr int kDotsAcrossALetter = 3;
    static constexpr int kRowsInALetter = 5;

    // Each letter is five rows of three dots, a row written as one number.
    static const uint8_t* rowsOf(char character) {
        static const uint8_t kLetters[26][5] = {
            {2, 5, 7, 5, 5}, {6, 5, 6, 5, 6}, {3, 4, 4, 4, 3}, {6, 5, 5, 5, 6}, {7, 4, 6, 4, 7}, {7, 4, 6, 4, 4}, {3, 4, 5, 5, 3},
            {5, 5, 7, 5, 5}, {7, 2, 2, 2, 7}, {1, 1, 1, 5, 2}, {5, 5, 6, 5, 5}, {4, 4, 4, 4, 7}, {5, 7, 7, 5, 5}, {6, 5, 5, 5, 5},
            {2, 5, 5, 5, 2}, {6, 5, 6, 4, 4}, {2, 5, 5, 6, 3}, {6, 5, 6, 5, 5}, {3, 4, 2, 1, 6}, {7, 2, 2, 2, 2}, {5, 5, 5, 5, 7},
            {5, 5, 5, 5, 2}, {5, 5, 7, 7, 5}, {5, 5, 2, 5, 5}, {5, 5, 2, 2, 2}, {7, 1, 2, 4, 7},
        };
        static const uint8_t kDigits[10][5] = {
            {7, 5, 5, 5, 7}, {2, 6, 2, 2, 7}, {6, 1, 2, 4, 7}, {6, 1, 2, 1, 6}, {5, 5, 7, 1, 1},
            {7, 4, 6, 1, 6}, {3, 4, 6, 5, 2}, {7, 1, 2, 2, 2}, {2, 5, 2, 5, 2}, {2, 5, 3, 1, 6},
        };
        static const uint8_t kMinus[5] = {0, 0, 7, 0, 0}, kPlus[5] = {0, 2, 7, 2, 0}, kColon[5] = {0, 2, 0, 2, 0}, kStop[5] = {0, 0, 0, 0, 2};
        static const uint8_t kSlash[5] = {1, 1, 2, 4, 4}, kUnderline[5] = {0, 0, 0, 0, 7};
        if (character >= 'a' && character <= 'z') return kLetters[character - 'a'];
        if (character >= 'A' && character <= 'Z') return kLetters[character - 'A'];
        if (character >= '0' && character <= '9') return kDigits[character - '0'];
        switch (character) {
            case '-': return kMinus;
            case '+': return kPlus;
            case ':': return kColon;
            case '.': return kStop;
            case '/': return kSlash;
            case '_': return kUnderline;
            default: return nullptr;
        }
    }

    void letter(char character, int x, int y, Colour colour, int size) {
        const uint8_t* rows = rowsOf(character);
        if (rows == nullptr) return;
        for (int row = 0; row < kRowsInALetter; ++row)
            for (int column = 0; column < kDotsAcrossALetter; ++column)
                if (rows[row] & (4 >> column)) fill(x + column * size, y + row * size, size, size, colour);
    }

    std::array<uint8_t, kByteCount> bytes_{};
};

// The lights of the device an app sets: the colour of each pad, and the buttons that glow.
struct Lights {
    std::array<Colour, kPads> pads{};
    std::array<bool, kButtons> buttons{};

    // Pads are numbered as on the device, from 1 at the bottom left to 16 at the top right.
    void pad(int number, Colour colour) {
        if (number >= 1 && number <= kPads) pads[static_cast<size_t>(number - 1)] = colour;
    }

    void glow(Button button, bool isLit = true) { buttons[static_cast<size_t>(button)] = isLit; }
};

// One short stretch of sound: what came in at the device's inputs, and where what goes out is written. Every
// value is between -1 and 1. An app that makes no sound of its own writes silence, or passes what came in on.
struct Sound {
    const float* inLeft;
    const float* inRight;
    float* outLeft;
    float* outRight;
    int frames;

    void silence() {
        std::fill(outLeft, outLeft + frames, 0.0f);
        std::fill(outRight, outRight + frames, 0.0f);
    }
};

// How MP Forge hands one sound file over: its name, and its sound for as long as the app runs. A file with one
// side only has the same sound for left and right.
struct SampleGiven {
    const char* name;
    const float* left;
    const float* right;
    int32_t frames;
};

// One sound file the musician added to the app, whole and ready to play. MP Forge has already brought it to the
// rate the app runs at, so reading one frame of it for every frame made plays it as it was recorded.
class Sample {
public:
    Sample() = default;

    explicit Sample(const SampleGiven& given) : left_(given.left), right_(given.right), frames_(given.frames) {
        std::strncpy(name_, given.name, kLongestSampleName);
    }

    // The file's name without its ending: `kick` for kick.wav.
    const char* name() const { return name_; }
    int frames() const { return frames_; }
    bool isStereo() const { return left_ != right_; }

    // The sound at one frame, between -1 and 1. Before the first frame and after the last there is silence, so
    // a voice that runs off either end of a sample plays nothing and harms nothing.
    float left(int frame) const { return isWithin(frame) ? left_[frame] : 0.0f; }
    float right(int frame) const { return isWithin(frame) ? right_[frame] : 0.0f; }

    // The sound at a place between two frames, for playing a sample faster, slower or backwards.
    float leftAt(double place) const { return between(&Sample::left, place); }
    float rightAt(double place) const { return between(&Sample::right, place); }

private:
    bool isWithin(int frame) const { return frame >= 0 && frame < frames_; }

    float between(float (Sample::*side)(int) const, double place) const {
        const double before = std::floor(place);
        if (before < -1.0 || before > static_cast<double>(frames_)) return 0.0f;
        const int frame = static_cast<int>(before);
        const float along = static_cast<float>(place - before);
        return (this->*side)(frame) + ((this->*side)(frame + 1) - (this->*side)(frame)) * along;
    }

    char name_[kLongestSampleName + 1] = {};
    const float* left_ = nullptr;
    const float* right_ = nullptr;
    int frames_ = 0;
};

// The sound files the musician added to the app, in the order of their names. An app that has been given none
// has a count of 0, and plays the sounds it makes in code or says on its screen that it wants some.
class Samples {
public:
    Samples() = default;

    Samples(const SampleGiven* given, int count) : count_(std::max(0, std::min(count, kMostSamples))) {
        for (int number = 0; number < count_; ++number) samples_[static_cast<size_t>(number)] = Sample(given[number]);
    }

    int count() const { return count_; }

    // For MP Forge: puts a sound at a number, in place of whatever was there.
    void put(int number, const SampleGiven& given) {
        if (number < 0 || number >= kMostSamples) return;
        samples_[static_cast<size_t>(number)] = Sample(given);
        count_ = std::max(count_, number + 1);
    }

    // Samples are numbered from 0. A number there is no sample at gives an empty one, which plays as silence.
    const Sample& at(int number) const {
        static const Sample kNoSample;
        return number >= 0 && number < count_ ? samples_[static_cast<size_t>(number)] : kNoSample;
    }

    // The sample of this name, whatever its capitals, or nullptr when the musician has added none called that.
    const Sample* named(const char* name) const {
        for (int number = 0; number < count_; ++number)
            if (isTheSameName(at(number).name(), name)) return &at(number);
        return nullptr;
    }

private:
    static bool isTheSameName(const char* one, const char* other) {
        for (; *one != '\0' && *other != '\0'; ++one, ++other)
            if (std::tolower(static_cast<unsigned char>(*one)) != std::tolower(static_cast<unsigned char>(*other))) return false;
        return *one == *other;
    }

    std::array<Sample, kMostSamples> samples_{};
    int count_ = 0;
};

// One thing in a folder of the card: a folder, or a sound file.
struct CardEntry {
    char name[kLongestSampleName + 1] = {};
    bool isFolder = false;
};

// How MP Forge hands over one thing it found in a folder of the card.
struct CardEntryGiven {
    const char* name;
    int32_t isFolder;
};

// What a card is asked for. MP Forge fetches it between two calls to the app, never while the app waits.
enum class Asked : int32_t { Nothing = 0, Folder = 1, File = 2 };

// What stands where the card goes: nothing, a card, or the computer. While an app runs in MP Forge there is no
// card to look through, and the musician picks a file on the computer for the kit's own page instead.
enum class CardIs : int32_t { Missing = 0, There = 1, PickedOnTheComputer = 2 };

// The SD card in the device, one folder at a time: the folders in it and its sound files. An app asks for a
// folder to be read or a file to be loaded, and goes on running. MP Forge answers a moment later: the folder's
// entries are then here to be read, or the file is one of the app's `samples()`.
class Card {
public:
    // Whether a card is in the device. Known once a folder has been read.
    bool isThere() const { return is_ == CardIs::There; }
    // Whether the app runs in MP Forge on a computer, where the kit's own page has a file picked there.
    bool isPickedOnTheComputer() const { return is_ == CardIs::PickedOnTheComputer; }
    // Whether something asked for has not been answered yet.
    bool isBusy() const { return asked_ != Asked::Nothing; }
    // The folder that was last read: empty for the top of the card, or such as DRUMS/808.
    const char* folder() const { return folder_; }
    int count() const { return count_; }

    const CardEntry& at(int number) const {
        static const CardEntry kNoEntry;
        return number >= 0 && number < count_ ? entries_[static_cast<size_t>(number)] : kNoEntry;
    }

    // Why the last file asked for was not loaded, in a few capitals for the screen. Empty when it was.
    const char* problem() const { return problem_; }

    // Reads the folder again, as when the app first looks at the card.
    void look() { ask(Asked::Folder, folder_); }

    void goInto(int number) {
        if (at(number).isFolder) ask(Asked::Folder, joined(at(number).name).words);
    }

    void goUp() {
        Path above;
        std::strncpy(above.words, folder_, kLongestCardPath);
        char* lastPart = std::strrchr(above.words, '/');
        if (lastPart != nullptr) *lastPart = '\0'; else above.words[0] = '\0';
        ask(Asked::Folder, above.words);
    }

    // Asks for a sound file of this folder to be loaded as sample `intoSample`, in place of what is there.
    void load(int number, int intoSample) {
        if (number < 0 || number >= count_ || at(number).isFolder) return;
        intoSample_ = intoSample;
        ask(Asked::File, joined(at(number).name).words);
    }

    // For MP Forge: what was asked for, which is then no longer waiting to be taken.
    Asked takeAsked(char* path, int32_t* intoSample) {
        const Asked taken = isTaken_ ? Asked::Nothing : asked_;
        if (taken == Asked::Nothing) return taken;
        std::strncpy(path, askedPath_, kLongestCardPath + 1);
        *intoSample = intoSample_;
        isTaken_ = true;
        return taken;
    }

    // For MP Forge: the folder that was asked for, or that there is no card or no such folder.
    void listed(const CardEntryGiven* found, int count, CardIs is) {
        is_ = is;
        count_ = isThere() ? std::max(0, std::min(count, kMostCardEntries)) : 0;
        for (int number = 0; number < count_; ++number) {
            CardEntry& entry = entries_[static_cast<size_t>(number)];
            entry = CardEntry{};
            std::strncpy(entry.name, found[number].name, kLongestSampleName);
            entry.isFolder = found[number].isFolder != 0;
        }
        std::strncpy(folder_, isThere() ? askedPath_ : "", kLongestCardPath);
        answered("");
    }

    // For MP Forge: the file that was asked for is loaded, or why it is not.
    void answered(const char* problem) {
        std::snprintf(problem_, sizeof problem_, "%s", problem);
        asked_ = Asked::Nothing;
        isTaken_ = false;
    }

private:
    struct Path {
        char words[kLongestCardPath + 1] = {};
    };

    // A name of this folder as a path from the top of the card.
    Path joined(const char* name) const {
        Path path;
        if (folder_[0] == '\0') std::snprintf(path.words, sizeof path.words, "%s", name);
        else std::snprintf(path.words, sizeof path.words, "%s/%s", folder_, name);
        return path;
    }

    // One thing is asked for at a time: while an answer is awaited, whatever else is asked for is let go.
    void ask(Asked what, const char* path) {
        if (isBusy()) return;
        std::snprintf(askedPath_, sizeof askedPath_, "%s", path);
        asked_ = what;
    }

    std::array<CardEntry, kMostCardEntries> entries_{};
    int count_ = 0;
    CardIs is_ = CardIs::Missing;
    char folder_[kLongestCardPath + 1] = {};
    char problem_[32] = {};
    Asked asked_ = Asked::Nothing;
    bool isTaken_ = false;
    char askedPath_[kLongestCardPath + 1] = {};
    int32_t intoSample_ = 0;
};

// What the kit's own pages have in common: their colours, the bar across the top that says what the buttons
// do, the rows of a list, and the words at the foot.
struct Page {
    static constexpr Colour kGround{16, 16, 24}, kWell{38, 37, 48}, kInk{236, 235, 242}, kSoft{120, 117, 135}, kAccent{240, 150, 48};
    static constexpr int kTopBar = 16, kUnderTheBar = 22, kRowsTop = 38, kRowHeight = 14, kNameLeft = 20, kRowsShown = 13;

    // One row of a list. The row the dial is on is lit, and a marked row has a block before its name.
    struct Row {
        const char* name;
        bool isChosen;
        bool isMarked;
    };

    static void drawTopBar(Screen& screen, const char* onTheLeft, const char* onTheRight) {
        screen.clear(kGround);
        screen.fill(0, 0, kScreenWidth, kTopBar, kWell);
        screen.write(onTheLeft, 6, 3, kInk, 2);
        screen.write(onTheRight, kScreenWidth - 6 - Screen::widthOf(onTheRight, 2), 3, kInk, 2);
    }

    static void drawRow(Screen& screen, int row, Row drawn) {
        const int y = kRowsTop + row * kRowHeight;
        if (drawn.isChosen) screen.fill(0, y - 2, kScreenWidth, kRowHeight, kInk);
        if (drawn.isMarked) screen.fill(8, y + 2, 7, 6, drawn.isChosen ? kGround : kAccent);
        screen.write(drawn.name, kNameLeft, y, drawn.isChosen ? kGround : kInk, 2);
    }

    static void drawFoot(Screen& screen, const char* words, Colour colour) { screen.write(words, 6, kScreenHeight - 10, colour); }

    // The row a list moves to when the dial turns, which is never past either end.
    static int movedTo(int row, int clicks, int count) { return std::max(0, std::min(row + clicks, count - 1)); }

    // The first row shown of a list too long for the screen, so that the row the dial is on stays in sight.
    static int firstShown(int chosen, int count) { return std::max(0, std::min(chosen - kRowsShown / 2, count - kRowsShown)); }
};

// The kit's own page for picking a sound file from the card. An app opens it with `browseCard`. While it is open
// it covers the screen, and the dial and two buttons work it: turning the dial moves through the folder,
// pressing the dial goes into a folder or loads a file, B1 goes back up a folder, and B3 closes the page.
// Everything else on the device still reaches the app, so the pads go on playing.
// While the app runs in MP Forge on a computer there is no card: opening the page has the musician pick a file
// on the computer, which is loaded the same way.
class Browser : private Page {
public:
    bool isOpen() const { return isOpen_; }
    int intoSample() const { return intoSample_; }

    void open(Card& card, int intoSample) {
        isOpen_ = true;
        intoSample_ = intoSample;
        chosen_ = 0;
        card.look();
    }

    void close() { isOpen_ = false; }

    static bool takes(Button button) { return button == Button::Encoder || button == Button::B1 || button == Button::B3; }

    void turned(const Card& card, int clicks) { chosen_ = movedTo(chosenIn(card), clicks, card.count()); }

    void pressed(Card& card, Button button) {
        if (button == Button::B3) return close();
        if (card.isBusy()) return;
        const int chosen = chosenIn(card);
        if (button == Button::B1) card.goUp();
        else if (card.at(chosen).isFolder) card.goInto(chosen);
        else card.load(chosen, intoSample_);
        if (button == Button::B1 || card.at(chosen).isFolder) chosen_ = 0;
    }

    void draw(Screen& screen, const Card& card) const {
        drawTopBar(screen, "B1 BACK", "B3 CLOSE");
        drawWhere(screen, card);
        if (card.isThere()) drawEntries(screen, card);
        drawFoot(screen, wordsAtTheFoot(card), card.problem()[0] != '\0' ? kAccent : kSoft);
    }

private:
    static constexpr int kLettersAcross = (kScreenWidth - kNameLeft - 4) / 8;

    int chosenIn(const Card& card) const { return std::max(0, std::min(chosen_, card.count() - 1)); }

    // Where on the card the page is, cut from the left when it is too long to fit.
    void drawWhere(Screen& screen, const Card& card) const {
        char where[kLongestCardPath + 8];
        std::snprintf(where, sizeof where, "CARD/%s", card.folder());
        const int tooMany = static_cast<int>(std::strlen(where)) - (kScreenWidth - 12) / 8;
        screen.write(where + std::max(0, tooMany), 6, kUnderTheBar, kSoft, 2);
    }

    void drawEntries(Screen& screen, const Card& card) const {
        const int chosen = chosenIn(card);
        const int first = firstShown(chosen, card.count());
        for (int row = 0; row < kRowsShown && first + row < card.count(); ++row) {
            const CardEntry& entry = card.at(first + row);
            char name[kLettersAcross + 1];
            std::snprintf(name, sizeof name, "%s", entry.name);
            drawRow(screen, row, {name, first + row == chosen, entry.isFolder});
        }
    }

    static const char* wordsAtTheFoot(const Card& card) {
        if (card.isBusy()) return "READING THE CARD";
        if (card.problem()[0] != '\0') return card.problem();
        if (card.isPickedOnTheComputer()) return "CHOOSE A FILE ON THE COMPUTER";
        if (!card.isThere()) return "NO SD CARD";
        if (card.count() == 0) return "NO FOLDERS OR SOUND FILES HERE";
        return "TURN THE DIAL TO MOVE. PRESS IT TO OPEN OR LOAD.";
    }

    bool isOpen_ = false;
    int intoSample_ = 0;
    int chosen_ = 0;
};

// Where the sound at the app's inputs comes from: the two jacks at the back of the device, its built-in
// microphone, whatever is plugged into its USB-C socket, or nowhere, which is silence.
// The numbers are the ones apps were built with, so a new place to listen is added at the end.
enum class SoundIn : int32_t { LineIn = 0, Usb = 1, Off = 2, Mic = 3 };
constexpr int kSoundIns = 4;

// The kit's own page for choosing where sound comes in from. An app opens it with `chooseSoundIn`. While it is
// open it covers the screen, and the dial and one button work it: turning the dial moves through the choices,
// pressing the dial takes the one it is on, and B3 closes the page. The one in use has a block before its
// name. Everything else on the device still reaches the app.
class SoundInPage : private Page {
public:
    bool isOpen() const { return isOpen_; }
    SoundIn chosen() const { return kChoices[chosen_].from; }

    void open(SoundIn inUse) {
        isOpen_ = true;
        chosen_ = rowOf(inUse);
    }

    void close() { isOpen_ = false; }

    static bool takes(Button button) { return button == Button::Encoder || button == Button::B3; }

    void turned(int clicks) { chosen_ = movedTo(chosen_, clicks, kSoundIns); }

    void draw(Screen& screen, SoundIn inUse) const {
        drawTopBar(screen, "SOUND IN", "B3 CLOSE");
        for (int row = 0; row < kSoundIns; ++row) drawRow(screen, row, {kChoices[row].name, row == chosen_, kChoices[row].from == inUse});
        drawFoot(screen, "TURN THE DIAL TO MOVE. PRESS IT TO CHOOSE.", kSoft);
    }

private:
    // The choices in the order the page lists them.
    struct Choice {
        SoundIn from;
        const char* name;
    };
    static constexpr Choice kChoices[kSoundIns] = {{SoundIn::LineIn, "LINE IN"}, {SoundIn::Mic, "MIC"}, {SoundIn::Usb, "USB"}, {SoundIn::Off, "OFF"}};

    static int rowOf(SoundIn from) {
        for (int row = 0; row < kSoundIns; ++row) if (kChoices[row].from == from) return row;
        return 0;
    }

    bool isOpen_ = false;
    int chosen_ = 0;
};

// Where the MIDI that plays the app comes in: at the device's USB-C socket and its MIDI IN socket together, at
// one of the two, or nowhere.
enum class MidiIn : int32_t { UsbAndSocket = 0, Usb = 1, Socket = 2, Off = 3 };
constexpr int kMidiIns = 4;
constexpr int kMidiChannels = 16;
// In place of a channel's number: every channel.
constexpr int kEveryChannel = 0;

// Where one MIDI message came in, as MP Forge says it. While an app runs in MP Forge on a computer, the
// computer's MIDI stands in for both of the device's sockets.
enum class MidiCameIn : int32_t { AtEither = 0, AtUsb = 1, AtTheSocket = 2 };

// The MIDI an app is played by: where it comes in, and on which channel, 1 to 16 or every one.
struct MidiHeard {
    MidiIn from = MidiIn::UsbAndSocket;
    int channel = kEveryChannel;

    bool takes(MidiCameIn cameIn, int onChannel) const {
        const bool isOnTheChannel = channel == kEveryChannel || channel == onChannel;
        return from != MidiIn::Off && listensWhere(cameIn) && isOnTheChannel;
    }

private:
    bool listensWhere(MidiCameIn cameIn) const {
        if (from == MidiIn::UsbAndSocket || cameIn == MidiCameIn::AtEither) return true;
        return (from == MidiIn::Usb) == (cameIn == MidiCameIn::AtUsb);
    }
};

// The kit's own page for choosing the MIDI an app is played by. An app opens it with `chooseMidiIn`. It is one
// list: first where MIDI comes in, then the channel. Turning the dial moves through it, pressing the dial takes
// the row it is on, and B3 closes the page. The source in use and the channel in use each have a block before
// their name, and both are written above the list, which is too long to show whole. Everything else on the
// device still reaches the app.
class MidiInPage : private Page {
public:
    bool isOpen() const { return isOpen_; }

    void open(MidiHeard inUse) {
        isOpen_ = true;
        chosen_ = rowOf(inUse.from);
    }

    void close() { isOpen_ = false; }

    static bool takes(Button button) { return button == Button::Encoder || button == Button::B3; }

    void turned(int clicks) { chosen_ = movedTo(chosen_, clicks, kRowCount); }

    // What is in use once the row the dial is on is taken: its source, or its channel, in place of the one before.
    MidiHeard taken(MidiHeard inUse) const {
        if (isASource(chosen_)) inUse.from = kSources[chosen_].from; else inUse.channel = channelOf(chosen_);
        return inUse;
    }

    void draw(Screen& screen, MidiHeard inUse) const {
        drawTopBar(screen, "MIDI IN", "B3 CLOSE");
        drawInUse(screen, inUse);
        const int first = firstShown(chosen_, kRowCount);
        for (int row = 0; row < kRowsShown; ++row) {
            const int listed = first + row;
            drawRow(screen, row, {nameOf(listed).words, listed == chosen_, isInUse(listed, inUse)});
        }
        drawFoot(screen, "TURN THE DIAL TO MOVE. PRESS IT TO CHOOSE.", kSoft);
    }

private:
    struct Source {
        MidiIn from;
        const char* name;
    };
    static constexpr Source kSources[kMidiIns] = {{MidiIn::UsbAndSocket, "USB AND MIDI IN"}, {MidiIn::Usb, "USB"}, {MidiIn::Socket, "MIDI IN"}, {MidiIn::Off, "OFF"}};
    // The sources, then every channel, then each channel by its number.
    static constexpr int kRowCount = kMidiIns + 1 + kMidiChannels;

    struct Name {
        char words[20] = {};
    };

    static void drawInUse(Screen& screen, MidiHeard inUse) {
        char words[2 * sizeof(Name) + 4];
        std::snprintf(words, sizeof words, "%s - %s", nameOf(rowOf(inUse.from)).words, nameOf(kMidiIns + inUse.channel).words);
        screen.write(words, 6, kUnderTheBar, kSoft, 2);
    }

    static bool isASource(int row) { return row < kMidiIns; }
    static int channelOf(int row) { return row - kMidiIns; }

    static int rowOf(MidiIn from) {
        for (int row = 0; row < kMidiIns; ++row) if (kSources[row].from == from) return row;
        return 0;
    }

    static bool isInUse(int row, MidiHeard inUse) { return isASource(row) ? kSources[row].from == inUse.from : channelOf(row) == inUse.channel; }

    static Name nameOf(int row) {
        Name name;
        if (isASource(row)) std::snprintf(name.words, sizeof name.words, "%s", kSources[row].name);
        else if (channelOf(row) == kEveryChannel) std::snprintf(name.words, sizeof name.words, "EVERY CHANNEL");
        else std::snprintf(name.words, sizeof name.words, "CHANNEL %d", channelOf(row));
        return name;
    }

    bool isOpen_ = false;
    int chosen_ = 0;
};

class App {
public:
    virtual ~App() = default;

    // Called once, before anything else, with how many frames of sound there are to a second. The samples are
    // already there to be read.
    virtual void start(double /*framesASecond*/) {}

    // Pads are numbered 1 to 16, from the bottom left. `strength` runs from just above 0 for the softest hit to 1.
    virtual void padHit(int /*pad*/, float /*strength*/) {}
    virtual void padLetGo(int /*pad*/) {}

    // The knobs are numbered 1 to 3. A knob and the fader say where they stand, from 0 to 1. They are told once
    // when the app starts, and then whenever they move.
    virtual void knobTurned(int /*knob*/, float /*stands*/) {}
    virtual void faderMoved(float /*stands*/) {}

    // The big dial has no end stops: it says how many clicks it turned, and which way (up is positive).
    virtual void encoderTurned(int /*clicks*/) {}

    virtual void buttonPressed(Button /*button*/) {}
    virtual void buttonLetGo(Button /*button*/) {}

    // MIDI from whatever is plugged into the device's USB socket or its MIDI IN: a keyboard, a pad controller,
    // a computer. Notes are numbered 0 to 127, with 60 at middle C, and channels 1 to 16. `strength` runs from
    // just above 0 to 1, `stands` from 0 to 1, and `bend` from -1 to 1 with 0 where the wheel rests. Only the
    // MIDI the app is played by gets here: see `midiIn`.
    virtual void noteOn(int /*note*/, float /*strength*/, int /*channel*/) {}
    virtual void noteOff(int /*note*/, int /*channel*/) {}
    virtual void controllerMoved(int /*controller*/, float /*stands*/, int /*channel*/) {}
    virtual void pitchBent(float /*bend*/, int /*channel*/) {}

    // A game controller plugged into the device's USB socket, one at a time. A stick says where it stands: `x`
    // from -1 at the left to 1 at the right, `y` from -1 at the bottom to 1 at the top, both 0 where it rests.
    // A trigger says how far it is pulled, from 0 to 1, and is told as a button as well.
    virtual void gamepadPressed(GamepadButton /*button*/) {}
    virtual void gamepadLetGo(GamepadButton /*button*/) {}
    virtual void gamepadStickMoved(GamepadSide /*stick*/, float /*x*/, float /*y*/) {}
    virtual void gamepadTriggerPulled(GamepadSide /*trigger*/, float /*pulled*/) {}

    virtual void makeSound(Sound& sound) = 0;

    // Called about thirty times a second. The screen starts as the app last left it, and so do the lights.
    virtual void draw(Screen& screen, Lights& lights) = 0;

    // The sound files the app has: the ones the musician added to it, and any loaded from the card since. A
    // sample changes only when a file is loaded from the card in its place, between two calls to the app. So
    // keep a sample by its number or by a pointer to it, never as a copy.
    const Samples& samples() const { return samples_; }

    // Called when a file from the card has been loaded as sample `number`, in place of what was there.
    virtual void sampleLoaded(int /*number*/) {}

    // The SD card in the device, for an app that draws its own way of looking through it.
    Card& card() { return card_; }

    // Opens the kit's own page for picking a sound file from the card, to be loaded as sample `intoSample`. The
    // page closes when a file has been loaded, or when the musician presses B3. See `Browser`.
    void browseCard(int intoSample) {
        closePages();
        browser_.open(card_, intoSample);
    }
    void closeBrowser() { browser_.close(); }
    bool isBrowsing() const { return browser_.isOpen(); }

    // Where the sound at the app's inputs comes from. It starts as `Off`, so an app that listens says where as
    // it starts, with `hearFrom`. MP Forge follows it: `inLeft` and `inRight` hold what comes in there, and
    // silence when it is `Off`.
    SoundIn soundIn() const { return soundIn_; }
    void hearFrom(SoundIn from) { soundIn_ = from; }

    // Opens the kit's own page for choosing where sound comes in from. It stays open until the musician
    // presses B3. See `SoundInPage`.
    void chooseSoundIn() {
        closePages();
        soundInPage_.open(soundIn_);
    }
    void closeSoundInPage() { soundInPage_.close(); }
    bool isChoosingSoundIn() const { return soundInPage_.isOpen(); }

    // Called when the musician has chosen, on the kit's page, where sound comes in from.
    virtual void soundInChosen(SoundIn /*from*/) {}

    // The MIDI the app is played by. It starts as both sockets and every channel. An app with a way of its own
    // to choose sets it with `hearMidi`.
    MidiHeard midiIn() const { return midiIn_; }
    void hearMidi(MidiHeard heard) { midiIn_ = heard; }

    // Opens the kit's own page for choosing the MIDI the app is played by. It stays open until the musician
    // presses B3. See `MidiInPage`.
    void chooseMidiIn() {
        closePages();
        midiInPage_.open(midiIn_);
    }
    void closeMidiInPage() { midiInPage_.close(); }
    bool isChoosingMidiIn() const { return midiInPage_.isOpen(); }

    // Called when the musician has chosen, on the kit's page, the MIDI the app is played by. A note held on
    // what is no longer listened to is never let go, so an app that holds notes lets them all go here.
    virtual void midiInChosen(MidiHeard /*heard*/) {}

    // For MP Forge, which hands the samples over before `start` is called, and a file from the card when it
    // has been loaded.
    void take(const Samples& samples) { samples_ = samples; }

    void takeFromCard(int number, const SampleGiven* loaded, const char* whyNot) {
        card_.answered(loaded != nullptr ? "" : whyNot);
        if (loaded == nullptr) return;
        samples_.put(number, *loaded);
        if (browser_.isOpen() && browser_.intoSample() == number) browser_.close();
        sampleLoaded(number);
    }

    // For MP Forge: the sample the kit's own page is open to load a file as, or -1 when it is not open. On a
    // computer that is when the musician is asked to pick a file there.
    int browsingInto() const { return browser_.isOpen() ? browser_.intoSample() : -1; }

    // For MP Forge: the dial and the buttons that one of the kit's pages takes while it is open. One page is
    // open at a time.
    bool pageTook(Button button, bool isPressed) {
        const bool isTaken = (browser_.isOpen() && Browser::takes(button)) || (soundInPage_.isOpen() && SoundInPage::takes(button)) ||
                             (midiInPage_.isOpen() && MidiInPage::takes(button));
        if (isTaken && isPressed) pressedOnThePage(button);
        return isTaken;
    }

    bool pageTookTheDial(int clicks) {
        if (browser_.isOpen()) browser_.turned(card_, clicks);
        if (soundInPage_.isOpen()) soundInPage_.turned(clicks);
        if (midiInPage_.isOpen()) midiInPage_.turned(clicks);
        return isOnAPage();
    }

    void drawPageOver(Screen& screen) const {
        if (browser_.isOpen()) browser_.draw(screen, card_);
        if (soundInPage_.isOpen()) soundInPage_.draw(screen, soundIn_);
        if (midiInPage_.isOpen()) midiInPage_.draw(screen, midiIn_);
    }

    // For MP Forge: a stick or a trigger of the game controller moved along one of its directions. A stick is
    // told to the app whole, with its other direction as it was last said.
    void gamepadMovedAlong(GamepadAxis axis, float stands) {
        if (static_cast<int>(axis) < 0 || static_cast<int>(axis) >= kGamepadAxes) return;
        gamepadStands_[static_cast<size_t>(axis)] = stands;
        if (axis == GamepadAxis::LeftTrigger) return gamepadTriggerPulled(GamepadSide::Left, stands);
        if (axis == GamepadAxis::RightTrigger) return gamepadTriggerPulled(GamepadSide::Right, stands);
        tellOfTheStick(axis == GamepadAxis::LeftX || axis == GamepadAxis::LeftY ? GamepadSide::Left : GamepadSide::Right);
    }

private:
    void tellOfTheStick(GamepadSide stick) {
        const GamepadAxis across = stick == GamepadSide::Left ? GamepadAxis::LeftX : GamepadAxis::RightX;
        const GamepadAxis upAndDown = stick == GamepadSide::Left ? GamepadAxis::LeftY : GamepadAxis::RightY;
        gamepadStickMoved(stick, gamepadStands_[static_cast<size_t>(across)], gamepadStands_[static_cast<size_t>(upAndDown)]);
    }

    bool isOnAPage() const { return browser_.isOpen() || soundInPage_.isOpen() || midiInPage_.isOpen(); }

    void closePages() {
        browser_.close();
        soundInPage_.close();
        midiInPage_.close();
    }

    void pressedOnThePage(Button button) {
        if (browser_.isOpen()) return browser_.pressed(card_, button);
        if (button == Button::B3) return closePages();
        if (soundInPage_.isOpen()) return takeTheSoundInChosen();
        takeTheMidiInChosen();
    }

    void takeTheSoundInChosen() {
        soundIn_ = soundInPage_.chosen();
        soundInChosen(soundIn_);
    }

    void takeTheMidiInChosen() {
        midiIn_ = midiInPage_.taken(midiIn_);
        midiInChosen(midiIn_);
    }

    Samples samples_;
    Card card_;
    Browser browser_;
    SoundIn soundIn_ = SoundIn::Off;
    SoundInPage soundInPage_;
    MidiHeard midiIn_;
    MidiInPage midiInPage_;
    std::array<float, kGamepadAxes> gamepadStands_{};
};

}  // namespace sample

// ---------------------------------------------------------------------------------------------------------
// How MP Forge reaches the app. One line at the end of an app's code names its class: MPC_SAMPLE_APP(MyApp)
// ---------------------------------------------------------------------------------------------------------

#if defined(_WIN32)
#define MP_FORGE_SAMPLE_SHOWN extern "C" __declspec(dllexport)
#else
#define MP_FORGE_SAMPLE_SHOWN extern "C" __attribute__((visibility("default")))
#endif

namespace sample {

// What MP Forge holds for a running app: the app, with the screen and the lights it draws on.
struct Running {
    App* app;
    Screen screen;
    Lights lights;
};

enum class Touched : int32_t { Pad = 0, Knob = 1, Fader = 2, Encoder = 3, Button = 4, Midi = 5, GamepadButton = 6, GamepadAxis = 7 };

// The kinds of MIDI message an app is told of, by the top half of a message's first byte.
enum class Midi : int32_t { NoteOff = 0x80, NoteOn = 0x90, Controller = 0xB0, PitchBend = 0xE0 };
constexpr float kMostInAMidiByte = 127.0f;
constexpr int kPitchBendAtRest = 8192;

// One MIDI message, its three bytes in one number with the first byte lowest, and above them where it came
// in. A message the app is not played by goes no further. A note on that is as soft as can be is the way many
// keyboards say a note was let go.
inline void tellOfMidi(App& app, int32_t message) {
    const int channel = (message & 0x0F) + 1;
    const int first = (message >> 8) & 0x7F, second = (message >> 16) & 0x7F;
    if (!app.midiIn().takes(static_cast<MidiCameIn>(message >> 24), channel)) return;
    switch (static_cast<Midi>(message & 0xF0)) {
        case Midi::NoteOn:
            if (second > 0) app.noteOn(first, static_cast<float>(second) / kMostInAMidiByte, channel); else app.noteOff(first, channel);
            return;
        case Midi::NoteOff: app.noteOff(first, channel); return;
        case Midi::Controller: app.controllerMoved(first, static_cast<float>(second) / kMostInAMidiByte, channel); return;
        case Midi::PitchBend: app.pitchBent(static_cast<float>((second << 7 | first) - kPitchBendAtRest) / kPitchBendAtRest, channel); return;
    }
}

inline void tell(App& app, int32_t touched, int32_t number, float value) {
    switch (static_cast<Touched>(touched)) {
        case Touched::Pad:
            if (number < 1 || number > kPads) return;
            if (value > 0.0f) app.padHit(number, value); else app.padLetGo(number);
            return;
        case Touched::Knob:
            if (number >= 1 && number <= kKnobs) app.knobTurned(number, value);
            return;
        case Touched::Fader: app.faderMoved(value); return;
        case Touched::Encoder:
            if (!app.pageTookTheDial(number)) app.encoderTurned(number);
            return;
        case Touched::Button:
            if (number < 0 || number >= kButtons || app.pageTook(static_cast<Button>(number), value > 0.0f)) return;
            if (value > 0.0f) app.buttonPressed(static_cast<Button>(number)); else app.buttonLetGo(static_cast<Button>(number));
            return;
        case Touched::Midi: tellOfMidi(app, number); return;
        case Touched::GamepadButton:
            if (number < 0 || number >= kGamepadButtons) return;
            if (value > 0.0f) app.gamepadPressed(static_cast<GamepadButton>(number)); else app.gamepadLetGo(static_cast<GamepadButton>(number));
            return;
        case Touched::GamepadAxis: app.gamepadMovedAlong(static_cast<GamepadAxis>(number), value); return;
    }
}

inline void show(Running& running, uint8_t* picture, uint8_t* padColours, uint8_t* litButtons) {
    running.app->draw(running.screen, running.lights);
    running.app->drawPageOver(running.screen);
    std::memcpy(picture, running.screen.bytes(), Screen::kByteCount);
    for (int pad = 0; pad < kPads; ++pad) {
        const Colour colour = running.lights.pads[static_cast<size_t>(pad)];
        padColours[pad * 3] = colour.red; padColours[pad * 3 + 1] = colour.green; padColours[pad * 3 + 2] = colour.blue;
    }
    for (int button = 0; button < kButtons; ++button) litButtons[button] = running.lights.buttons[static_cast<size_t>(button)] ? 1 : 0;
}

}  // namespace sample

#define MPC_SAMPLE_APP(AppClass)                                                                                  \
    MP_FORGE_SAMPLE_SHOWN int32_t mp_forge_sample_kit() { return 6; }                                             \
    MP_FORGE_SAMPLE_SHOWN void* mp_forge_sample_make(double framesASecond, const sample::SampleGiven* samples,    \
                                                     int32_t sampleCount) {                                       \
        auto* running = new sample::Running{new AppClass(), {}, {}};                                              \
        running->app->take(sample::Samples(samples, sampleCount));                                                \
        running->app->start(framesASecond);                                                                       \
        return running;                                                                                           \
    }                                                                                                             \
    MP_FORGE_SAMPLE_SHOWN void mp_forge_sample_free(void* made) {                                                 \
        auto* running = static_cast<sample::Running*>(made);                                                      \
        delete running->app;                                                                                      \
        delete running;                                                                                           \
    }                                                                                                             \
    MP_FORGE_SAMPLE_SHOWN void mp_forge_sample_told(void* made, int32_t touched, int32_t number, float value) {   \
        sample::tell(*static_cast<sample::Running*>(made)->app, touched, number, value);                          \
    }                                                                                                             \
    MP_FORGE_SAMPLE_SHOWN void mp_forge_sample_sound(void* made, const float* inLeft, const float* inRight,       \
                                                     float* outLeft, float* outRight, int32_t frames) {           \
        sample::Sound sound{inLeft, inRight, outLeft, outRight, frames};                                          \
        static_cast<sample::Running*>(made)->app->makeSound(sound);                                               \
    }                                                                                                             \
    MP_FORGE_SAMPLE_SHOWN void mp_forge_sample_show(void* made, uint8_t* picture, uint8_t* padColours,            \
                                                    uint8_t* litButtons) {                                        \
        sample::show(*static_cast<sample::Running*>(made), picture, padColours, litButtons);                      \
    }                                                                                                             \
    MP_FORGE_SAMPLE_SHOWN int32_t mp_forge_sample_asked(void* made, char* path, int32_t* intoSample) {            \
        return static_cast<int32_t>(static_cast<sample::Running*>(made)->app->card().takeAsked(path, intoSample)); \
    }                                                                                                             \
    MP_FORGE_SAMPLE_SHOWN void mp_forge_sample_listed(void* made, const sample::CardEntryGiven* found,            \
                                                      int32_t count, int32_t cardIs) {                            \
        static_cast<sample::Running*>(made)->app->card().listed(found, count, static_cast<sample::CardIs>(cardIs)); \
    }                                                                                                             \
    MP_FORGE_SAMPLE_SHOWN void mp_forge_sample_loaded(void* made, int32_t number, const sample::SampleGiven* loaded, \
                                                      const char* whyNot) {                                       \
        static_cast<sample::Running*>(made)->app->takeFromCard(number, loaded, whyNot);                           \
    }                                                                                                             \
    MP_FORGE_SAMPLE_SHOWN int32_t mp_forge_sample_sound_in(void* made) {                                          \
        return static_cast<int32_t>(static_cast<sample::Running*>(made)->app->soundIn());                         \
    }                                                                                                             \
    MP_FORGE_SAMPLE_SHOWN int32_t mp_forge_sample_browsing_into(void* made) {                                     \
        return static_cast<sample::Running*>(made)->app->browsingInto();                                          \
    }                                                                                                             \
    MP_FORGE_SAMPLE_SHOWN void mp_forge_sample_close_browser(void* made) {                                        \
        static_cast<sample::Running*>(made)->app->closeBrowser();                                                 \
    }
