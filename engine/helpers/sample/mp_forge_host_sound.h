// Where the sound of an app goes on the device: the MPC Sample's own sound card, reached with nothing but
// what Linux itself offers, so the program needs no library of the device's.
//
// The card is asked for 44,100 frames a second on two sides, in short stretches so that a pad is heard at
// once. Where the sound's place is no sound card, as when this program stands in for the device on a
// computer, the same numbers are written to it as a file. Where it cannot be opened at all the sound is let
// go at the pace it would have been played, so an app that counts time by its sound still keeps time.

#pragma once

#include <algorithm>
#include <cerrno>
#include <chrono>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <fcntl.h>
#include <sys/stat.h>
#include <thread>
#include <unistd.h>
#include <vector>

#if defined(__linux__)
#include <sound/asound.h>
#include <sys/ioctl.h>
#endif

namespace mp_forge_host {

constexpr int kFramesASecond = 44100;
constexpr int kSides = 2;

#if defined(__linux__)

// The switches and levels of the sound card. They are read into the notes of a run, and one is set: the
// built-in speaker is switched on when it is off and no headphones are in, or nothing would be heard.
class Mixer {
public:
    explicit Mixer(const char* path) : file_(open(path, O_RDWR | O_CLOEXEC)) {}
    ~Mixer() { if (file_ >= 0) close(file_); }

    void letTheSpeakerBeHeard() {
        long speaker = 0, headphones = 0;
        if (!read("Builtin Speaker Switch", speaker) || speaker != 0) return;
        if (read("Headphones Jack", headphones) && headphones != 0) return;
        const bool isOn = write("Builtin Speaker Switch", 1);
        std::fprintf(stderr, "The built-in speaker was off with no headphones in. Switching it on %s.\n", isOn ? "worked" : "did not work");
    }

    // Writes how every switch and level stands, for whoever reads the notes of a run that stayed silent.
    void noteHowItStands() {
        if (file_ < 0) return (void)std::fprintf(stderr, "The sound card's switches could not be opened.\n");
        for (const snd_ctl_elem_id& id : everySwitch()) noteOne(id);
    }

private:
    static constexpr unsigned kMostSwitches = 256;

    std::vector<snd_ctl_elem_id> everySwitch() {
        std::vector<snd_ctl_elem_id> ids(kMostSwitches);
        snd_ctl_elem_list list{};
        list.space = kMostSwitches;
        list.pids = ids.data();
        if (file_ < 0 || ioctl(file_, SNDRV_CTL_IOCTL_ELEM_LIST, &list) < 0) return {};
        ids.resize(list.used);
        return ids;
    }

    bool find(const char* name, snd_ctl_elem_id& found) {
        for (const snd_ctl_elem_id& id : everySwitch())
            if (std::strcmp(reinterpret_cast<const char*>(id.name), name) == 0) { found = id; return true; }
        return false;
    }

    static bool isAWholeNumber(const snd_ctl_elem_info& info) {
        return info.type == SNDRV_CTL_ELEM_TYPE_BOOLEAN || info.type == SNDRV_CTL_ELEM_TYPE_INTEGER;
    }

    bool read(const char* name, long& value) {
        snd_ctl_elem_value held{};
        if (!find(name, held.id) || ioctl(file_, SNDRV_CTL_IOCTL_ELEM_READ, &held) < 0) return false;
        value = held.value.integer.value[0];
        return true;
    }

    bool write(const char* name, long value) {
        snd_ctl_elem_value held{};
        if (!find(name, held.id)) return false;
        held.value.integer.value[0] = value;
        return ioctl(file_, SNDRV_CTL_IOCTL_ELEM_WRITE, &held) >= 0;
    }

    void noteOne(const snd_ctl_elem_id& id) {
        snd_ctl_elem_info info{};
        snd_ctl_elem_value held{};
        info.id = id;
        held.id = id;
        if (ioctl(file_, SNDRV_CTL_IOCTL_ELEM_INFO, &info) < 0 || !isAWholeNumber(info) || ioctl(file_, SNDRV_CTL_IOCTL_ELEM_READ, &held) < 0) return;
        std::fprintf(stderr, "  %s =", reinterpret_cast<const char*>(id.name));
        for (unsigned part = 0; part < info.count && part < 4; ++part) std::fprintf(stderr, " %ld", held.value.integer.value[part]);
        if (info.type == SNDRV_CTL_ELEM_TYPE_INTEGER) std::fprintf(stderr, " (%ld to %ld)", info.value.integer.min, info.value.integer.max);
        std::fprintf(stderr, "\n");
    }

    int file_;
};

// What a sound card is asked for before it plays: every part of it left open but the ones set here.
class AskedOfTheCard {
public:
    AskedOfTheCard(int framesAStretch, int stretchesWaiting) {
        for (int mask = SNDRV_PCM_HW_PARAM_FIRST_MASK; mask <= SNDRV_PCM_HW_PARAM_LAST_MASK; ++mask) leaveOpen(asked_.masks[mask - SNDRV_PCM_HW_PARAM_FIRST_MASK]);
        for (int range = SNDRV_PCM_HW_PARAM_FIRST_INTERVAL; range <= SNDRV_PCM_HW_PARAM_LAST_INTERVAL; ++range) leaveOpen(asked_.intervals[range - SNDRV_PCM_HW_PARAM_FIRST_INTERVAL]);
        asked_.rmask = ~0u;
        asked_.info = ~0u;
        choose(SNDRV_PCM_HW_PARAM_ACCESS, SNDRV_PCM_ACCESS_RW_INTERLEAVED);
        choose(SNDRV_PCM_HW_PARAM_FORMAT, SNDRV_PCM_FORMAT_S32_LE);
        choose(SNDRV_PCM_HW_PARAM_SUBFORMAT, SNDRV_PCM_SUBFORMAT_STD);
        set(SNDRV_PCM_HW_PARAM_CHANNELS, kSides);
        set(SNDRV_PCM_HW_PARAM_RATE, kFramesASecond);
        set(SNDRV_PCM_HW_PARAM_PERIOD_SIZE, static_cast<unsigned>(framesAStretch));
        set(SNDRV_PCM_HW_PARAM_PERIODS, static_cast<unsigned>(stretchesWaiting));
    }

    snd_pcm_hw_params* asked() { return &asked_; }

private:
    static void leaveOpen(snd_mask& mask) { std::memset(mask.bits, 0xFF, sizeof mask.bits); }

    static void leaveOpen(snd_interval& range) {
        range.min = 0;
        range.max = ~0u;
    }

    void choose(int which, int chosen) {
        snd_mask& mask = asked_.masks[which - SNDRV_PCM_HW_PARAM_FIRST_MASK];
        std::memset(mask.bits, 0, sizeof mask.bits);
        mask.bits[chosen >> 5] |= 1u << (chosen & 31);
    }

    void set(int which, unsigned value) {
        snd_interval& range = asked_.intervals[which - SNDRV_PCM_HW_PARAM_FIRST_INTERVAL];
        range.min = value;
        range.max = value;
        range.integer = 1;
    }

    snd_pcm_hw_params asked_{};
};

// The sound card, opened and made ready to be written to.
class SoundCard {
public:
    // The shortest stretches first, for the least wait between a pad and its sound.
    bool open(int file) {
        for (const int framesAStretch : {128, 256, 512})
            if (isMadeReady(file, framesAStretch)) return true;
        return false;
    }

    int framesAStretch() const { return framesAStretch_; }

    // A card that ran out of sound, or was put to sleep, is made ready again and written to once more.
    bool play(int file, const int32_t* frames, int frameCount) {
        for (int tries = 0; tries < 2; ++tries) {
            snd_xferi sent{};
            sent.buf = const_cast<int32_t*>(frames);
            sent.frames = static_cast<snd_pcm_uframes_t>(frameCount);
            if (ioctl(file, SNDRV_PCM_IOCTL_WRITEI_FRAMES, &sent) >= 0) return true;
            if (errno != EPIPE && errno != ESTRPIPE && errno != EINTR) return false;
            ioctl(file, SNDRV_PCM_IOCTL_PREPARE);
        }
        return true;
    }

private:
    static constexpr int kStretchesWaiting = 4;

    bool isMadeReady(int file, int framesAStretch) {
        AskedOfTheCard hardware(framesAStretch, kStretchesWaiting);
        if (ioctl(file, SNDRV_PCM_IOCTL_HW_PARAMS, hardware.asked()) < 0) return false;
        snd_pcm_sw_params software{};
        software.tstamp_mode = SNDRV_PCM_TSTAMP_NONE;
        software.period_step = 1;
        software.avail_min = static_cast<snd_pcm_uframes_t>(framesAStretch);
        software.start_threshold = static_cast<snd_pcm_uframes_t>(framesAStretch);
        software.stop_threshold = static_cast<snd_pcm_uframes_t>(framesAStretch) * kStretchesWaiting;
        software.boundary = software.stop_threshold;
        if (ioctl(file, SNDRV_PCM_IOCTL_SW_PARAMS, &software) < 0 || ioctl(file, SNDRV_PCM_IOCTL_PREPARE) < 0) return false;
        framesAStretch_ = framesAStretch;
        return true;
    }

    int framesAStretch_ = 0;
};

#endif

// Where an app's sound goes, a stretch at a time.
class SoundOut {
public:
    static constexpr int kFramesAStretchOffTheDevice = 128;

    // `place` is the sound card on the device. `mixer` is where that card's switches are.
    SoundOut(const char* place, const char* mixer) : file_(open(place, O_RDWR | O_CLOEXEC)) {
        if (file_ < 0) std::fprintf(stderr, "The sound could not be opened at %s (%s). The app runs without sound.\n", place, std::strerror(errno));
        if (file_ >= 0 && isADevice()) takeTheSoundCard(mixer);
    }

    int framesAStretch() const { return framesAStretch_; }

    // Plays one stretch, as loud as `volume` says from 0 to 1. Sound that is not a number, or louder than
    // full, never reaches the card as it is.
    void play(const float* left, const float* right, float volume) {
        frames_.resize(static_cast<size_t>(framesAStretch_) * kSides);
        for (int frame = 0; frame < framesAStretch_; ++frame) {
            frames_[static_cast<size_t>(frame) * kSides] = fitToHear(left[frame] * volume);
            frames_[static_cast<size_t>(frame) * kSides + 1] = fitToHear(right[frame] * volume);
        }
        if (!isASoundCard_) return writeAtThePaceOfSound();
#if defined(__linux__)
        if (!card_.play(file_, frames_.data(), framesAStretch_)) stopUsingTheCard();
#endif
    }

private:
    static constexpr double kLoudest = 2147483647.0;

    static int32_t fitToHear(float made) {
        if (!std::isfinite(made)) return 0;
        return static_cast<int32_t>(std::lround(static_cast<double>(std::max(-1.0f, std::min(made, 1.0f))) * kLoudest));
    }

    bool isADevice() const {
        struct stat found {};
        return fstat(file_, &found) == 0 && S_ISCHR(found.st_mode);
    }

    void takeTheSoundCard([[maybe_unused]] const char* mixer) {
#if defined(__linux__)
        Mixer switches(mixer);
        std::fprintf(stderr, "How the sound card's switches stand:\n");
        switches.noteHowItStands();
        switches.letTheSpeakerBeHeard();
        isASoundCard_ = card_.open(file_);
        if (isASoundCard_) framesAStretch_ = card_.framesAStretch();
        std::fprintf(stderr, isASoundCard_ ? "The sound card plays %d frames at a time.\n" : "The sound card took none of the ways it was asked to play (%d). The app runs without sound.\n",
                     isASoundCard_ ? framesAStretch_ : errno);
#endif
        if (!isASoundCard_) stopUsingTheCard();
    }

    void stopUsingTheCard() {
        if (isASoundCard_) std::fprintf(stderr, "The sound card stopped taking sound (%s).\n", std::strerror(errno));
        isASoundCard_ = false;
        close(file_);
        file_ = -1;
    }

    // Off the device the sound is written as it would be played, and no faster. With nowhere to write it is let go.
    void writeAtThePaceOfSound() {
        if (file_ >= 0 && write(file_, frames_.data(), frames_.size() * sizeof(int32_t)) < 0) stopUsingTheCard();
        const auto aStretch = std::chrono::microseconds(1000000LL * framesAStretch_ / kFramesASecond);
        nextStretch_ = std::max(nextStretch_ + aStretch, std::chrono::steady_clock::now() - 8 * aStretch);
        std::this_thread::sleep_until(nextStretch_);
    }

    int file_;
    bool isASoundCard_ = false;
    int framesAStretch_ = kFramesAStretchOffTheDevice;
    std::vector<int32_t> frames_;
    std::chrono::steady_clock::time_point nextStretch_ = std::chrono::steady_clock::now();
#if defined(__linux__)
    SoundCard card_;
#endif
};

}  // namespace mp_forge_host
