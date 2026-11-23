// The files the program that runs an app on the device reads for it: the sound files the musician added to
// the app, and the SD card's folders and sound files.
//
// An app never opens a file. It is handed its sounds whole, and asks for a folder of the card to be read or a
// file of it to be loaded. Here those are read the way MP Forge reads them on a computer, so an app hears the
// same sound from the same file on both. Nothing is ever written, and no path an app asks for leaves the card.

#pragma once

#include "mpc_sample_app.h"

#include <algorithm>
#include <cctype>
#include <climits>
#include <cmath>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <dirent.h>
#include <fstream>
#include <string>
#include <sys/stat.h>
#include <vector>

namespace mp_forge_host {

// As much sound as an app holds at once, counted in numbers, and the largest file it is given.
constexpr size_t kMostNumbersOfSoundAnAppHolds = 64u * 1024u * 1024u;
constexpr size_t kLargestSoundFileInBytes = 16u * 1024u * 1024u;

// Why a file was not loaded, in the few capitals the device's screen has room for.
constexpr const char* kNoCard = "NO SD CARD";
constexpr const char* kNotFound = "THAT FILE HAS GONE";
constexpr const char* kTooLarge = "TOO LARGE: 16 MB AT MOST";
constexpr const char* kNotAReadableWav = "NOT A WAV THAT CAN BE READ";
constexpr const char* kNoMp3OnTheDeviceYet = "MP3 NOT YET ON THE DEVICE";
constexpr const char* kNoRoomForIt = "NO ROOM LEFT FOR IT";

// One sound, whole, at the rate the app runs at. A file with one side only has no sound of its own for the right.
struct HeldSound {
    std::string name;
    std::vector<float> left;
    std::vector<float> right;

    size_t numbersHeld() const { return left.size() + right.size(); }

    // As the kit takes it. What it points at stays where it is for as long as this sound is held.
    sample::SampleGiven given() const {
        const float* rightSide = right.empty() ? left.data() : right.data();
        return sample::SampleGiven{name.c_str(), left.data(), rightSide, static_cast<int32_t>(left.size())};
    }
};

inline bool endsWith(const std::string& name, const char* ending) {
    const size_t length = std::strlen(ending);
    if (name.size() < length) return false;
    return std::equal(name.end() - static_cast<std::ptrdiff_t>(length), name.end(), ending,
                      [](char one, char other) { return std::tolower(static_cast<unsigned char>(one)) == other; });
}

inline std::string withoutEnding(const std::string& fileName) {
    const size_t dot = fileName.rfind('.');
    return dot == std::string::npos ? fileName : fileName.substr(0, dot);
}

inline std::string lastPartOf(const std::string& path) {
    const size_t slash = path.rfind('/');
    return slash == std::string::npos ? path : path.substr(slash + 1);
}

inline bool isAFolder(const std::string& path) {
    struct stat found {};
    return stat(path.c_str(), &found) == 0 && S_ISDIR(found.st_mode);
}

// A WAV file's sound, read the way MP Forge reads it on a computer: mono or stereo, whole numbers of 8 to 32
// bits or floating point of 32.
class Wav {
public:
    explicit Wav(std::vector<uint8_t> bytes) : bytes_(std::move(bytes)) { readTheHeader(); }

    bool canBeRead() const { return canBeRead_; }

    // The sound at the rate an app runs at, each new frame read off the line between the two it falls between.
    HeldSound soundAt(double framesASecond, const std::string& name) const {
        HeldSound sound;
        sound.name = name;
        const double step = static_cast<double>(framesASecondRecorded_) / framesASecond;
        const size_t framesOut = step == 1.0 ? frames_ : static_cast<size_t>(std::floor(static_cast<double>(frames_) / step));
        sound.left.resize(framesOut);
        if (channels_ > 1) sound.right.resize(framesOut);
        for (size_t frame = 0; frame < framesOut; ++frame) {
            sound.left[frame] = between(0, static_cast<double>(frame) * step);
            if (channels_ > 1) sound.right[frame] = between(channels_ - 1, static_cast<double>(frame) * step);
        }
        return sound;
    }

private:
    static constexpr size_t kOpeningLength = 12, kChunkLabelLength = 8, kLeastFormatLength = 16;
    static constexpr uint16_t kWholeNumbers = 1, kFloatingPoint = 3, kExtensible = 0xFFFE;

    uint16_t u16At(size_t at) const { return static_cast<uint16_t>(bytes_[at] | bytes_[at + 1] << 8); }
    uint32_t u32At(size_t at) const { return static_cast<uint32_t>(u16At(at)) | static_cast<uint32_t>(u16At(at + 2)) << 16; }

    void readTheHeader() {
        if (bytes_.size() < kOpeningLength || std::memcmp(bytes_.data(), "RIFF", 4) != 0 || std::memcmp(&bytes_[8], "WAVE", 4) != 0) return;
        size_t formatAt = 0, formatLength = 0;
        bool hasSound = false;
        for (size_t at = kOpeningLength; at + kChunkLabelLength <= bytes_.size();) {
            const size_t statedLength = u32At(at + 4);
            const size_t bodyAt = at + kChunkLabelLength;
            const size_t bodyEnd = std::min(bodyAt + statedLength, bytes_.size());
            if (formatLength == 0 && std::memcmp(&bytes_[at], "fmt ", 4) == 0) { formatAt = bodyAt; formatLength = bodyEnd - bodyAt; }
            if (!hasSound && std::memcmp(&bytes_[at], "data", 4) == 0) { soundAt_ = bodyAt; soundLength_ = bodyEnd - bodyAt; hasSound = true; }
            at = bodyEnd + statedLength % 2;
        }
        canBeRead_ = hasSound && formatLength >= kLeastFormatLength && takeTheFormat(formatAt, formatLength);
    }

    bool takeTheFormat(size_t at, size_t length) {
        const uint16_t stated = u16At(at);
        const uint16_t kind = stated == kExtensible && length >= 26 ? u16At(at + 24) : stated;
        channels_ = u16At(at + 2);
        framesASecondRecorded_ = u32At(at + 4);
        bytesANumber_ = u16At(at + 14) / 8u;
        isFloatingPoint_ = kind == kFloatingPoint;
        const bool isAKindThatIsRead = isFloatingPoint_ ? bytesANumber_ == 4 : kind == kWholeNumbers && bytesANumber_ >= 1 && bytesANumber_ <= 4;
        const bool hasWholeBytes = u16At(at + 14) % 8 == 0;
        if (!isAKindThatIsRead || !hasWholeBytes || channels_ < 1 || channels_ > 2 || framesASecondRecorded_ == 0) return false;
        frames_ = soundLength_ / (bytesANumber_ * channels_);
        return frames_ > 0;
    }

    // One number of the sound. Whole numbers are written smallest byte first, and 8-bit ones count up from nought.
    float numberAt(size_t frame, size_t channel) const {
        const uint8_t* written = &bytes_[soundAt_ + (frame * channels_ + channel) * bytesANumber_];
        if (isFloatingPoint_) {
            float number;
            std::memcpy(&number, written, sizeof number);
            return number;
        }
        switch (bytesANumber_) {
            case 1: return (static_cast<float>(written[0]) - 128.0f) / 128.0f;
            case 2: return static_cast<float>(static_cast<int16_t>(written[0] | written[1] << 8)) / 32768.0f;
            case 3: return static_cast<float>(static_cast<int32_t>(static_cast<uint32_t>(written[0]) << 8 | static_cast<uint32_t>(written[1]) << 16 | static_cast<uint32_t>(written[2]) << 24) >> 8) / 8388608.0f;
            default: return static_cast<float>(static_cast<int32_t>(u32At(static_cast<size_t>(written - bytes_.data())))) / 2147483648.0f;
        }
    }

    float between(size_t channel, double place) const {
        const size_t last = frames_ - 1;
        const size_t before = std::min(static_cast<size_t>(std::floor(place)), last);
        const size_t after = std::min(before + 1, last);
        const float along = static_cast<float>(place - std::floor(place));
        const float from = numberAt(before, channel);
        return from + (numberAt(after, channel) - from) * along;
    }

    std::vector<uint8_t> bytes_;
    bool canBeRead_ = false;
    size_t soundAt_ = 0, soundLength_ = 0, frames_ = 0, channels_ = 0, bytesANumber_ = 0;
    uint32_t framesASecondRecorded_ = 0;
    bool isFloatingPoint_ = false;
};

// Reads one sound file whole and brings it to the rate the app runs at. Answers with why not, or with nothing
// when `into` holds the sound.
inline const char* readAsSound(const std::string& file, double framesASecond, HeldSound& into) {
    struct stat found {};
    if (stat(file.c_str(), &found) != 0 || !S_ISREG(found.st_mode)) return kNotFound;
    if (static_cast<size_t>(found.st_size) > kLargestSoundFileInBytes) return kTooLarge;
    if (endsWith(file, ".mp3")) return kNoMp3OnTheDeviceYet;
    std::ifstream opened(file, std::ios::binary);
    std::vector<uint8_t> bytes(static_cast<size_t>(found.st_size));
    if (!opened.read(reinterpret_cast<char*>(bytes.data()), static_cast<std::streamsize>(bytes.size()))) return kNotFound;
    const Wav wav(std::move(bytes));
    if (!wav.canBeRead()) return kNotAReadableWav;
    into = wav.soundAt(framesASecond, withoutEnding(lastPartOf(file)));
    return nullptr;
}

// The names of what a folder holds. Empty when there is no such folder.
inline std::vector<std::string> namesIn(const std::string& folder) {
    std::vector<std::string> names;
    DIR* opened = opendir(folder.c_str());
    if (opened == nullptr) return names;
    while (const dirent* found = readdir(opened)) names.emplace_back(found->d_name);
    closedir(opened);
    return names;
}

// The sound files the musician added to the app, in the order of their names, as many as the kit holds and
// no more sound than an app has room for. A file that cannot be read is left out.
inline std::vector<HeldSound> soundsAddedToTheApp(const std::string& folder, double framesASecond) {
    std::vector<std::string> names = namesIn(folder);
    names.erase(std::remove_if(names.begin(), names.end(), [](const std::string& name) { return name[0] == '.' || !endsWith(name, ".wav"); }), names.end());
    std::sort(names.begin(), names.end());
    std::vector<HeldSound> sounds;
    size_t numbersHeld = 0;
    for (const std::string& name : names) {
        HeldSound sound;
        if (sounds.size() >= static_cast<size_t>(sample::kMostSamples) || readAsSound(folder + "/" + name, framesASecond, sound) != nullptr) continue;
        if (numbersHeld + sound.numbersHeld() > kMostNumbersOfSoundAnAppHolds) continue;
        numbersHeld += sound.numbersHeld();
        sounds.push_back(std::move(sound));
    }
    return sounds;
}

// One thing in a folder of the card, as the card's page of the kit lists it.
struct Listed {
    std::string name;
    bool isFolder;
};

// The SD card, from its top folder down. A card whose top folder is not there is no card.
class SdCard {
public:
    explicit SdCard(const char* top) {
        char resolved[PATH_MAX];
        if (realpath(top, resolved) != nullptr && isAFolder(resolved)) top_ = resolved;
    }

    bool isThere() const { return !top_.empty(); }

    // The folders and sound files of a folder, folders first, each in the order of their names. False when
    // there is no card or no such folder on it.
    bool list(const char* folder, std::vector<Listed>& entries) const {
        const std::string onTheCard = whereOnTheCard(folder);
        if (onTheCard.empty() || !isAFolder(onTheCard)) return false;
        for (const std::string& name : namesIn(onTheCard)) {
            const bool isFolder = isAFolder(onTheCard + "/" + name);
            if (isShown(name, isFolder)) entries.push_back({name, isFolder});
        }
        std::sort(entries.begin(), entries.end(), comesBefore);
        if (entries.size() > static_cast<size_t>(sample::kMostCardEntries)) entries.resize(static_cast<size_t>(sample::kMostCardEntries));
        return true;
    }

    const char* load(const char* file, double framesASecond, HeldSound& into) const {
        if (!isThere()) return kNoCard;
        const std::string onTheCard = whereOnTheCard(file);
        return onTheCard.empty() ? kNotFound : readAsSound(onTheCard, framesASecond, into);
    }

private:
    // Where a path asked for lies, when it lies on the card. A path that leaves the card, by its own steps or
    // through a link, lies nowhere.
    std::string whereOnTheCard(const char* path) const {
        if (!isThere() || !isMadeOfPlainSteps(path)) return "";
        char resolved[PATH_MAX];
        if (realpath((top_ + "/" + path).c_str(), resolved) == nullptr) return "";
        const std::string found = resolved;
        const bool isOnTheCard = found == top_ || found.compare(0, top_.size() + 1, top_ + "/") == 0;
        return isOnTheCard ? found : "";
    }

    static bool isMadeOfPlainSteps(const std::string& path) {
        if (path.empty()) return true;
        size_t from = 0;
        for (;;) {
            const size_t slash = path.find('/', from);
            const std::string step = path.substr(from, slash == std::string::npos ? std::string::npos : slash - from);
            if (step.empty() || step == "." || step == "..") return false;
            if (slash == std::string::npos) return true;
            from = slash + 1;
        }
    }

    // Hidden files, files that are not WAV or MP3 sound, and names too long for the kit are left out.
    static bool isShown(const std::string& name, bool isFolder) {
        const bool isSound = endsWith(name, ".wav") || endsWith(name, ".mp3");
        return name[0] != '.' && name.size() <= static_cast<size_t>(sample::kLongestSampleName) && (isFolder || isSound);
    }

    static std::string inSmallLetters(std::string name) {
        std::transform(name.begin(), name.end(), name.begin(), [](unsigned char letter) { return static_cast<char>(std::tolower(letter)); });
        return name;
    }

    static bool comesBefore(const Listed& one, const Listed& other) {
        if (one.isFolder != other.isFolder) return one.isFolder;
        return inSmallLetters(one.name) < inSmallLetters(other.name);
    }

    std::string top_;
};

}  // namespace mp_forge_host
