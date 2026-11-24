// What an app hears on the device: the MPC Sample's built-in microphone or its line in, read from the same
// sound card its sound is played on.
//
// The app says where it listens, and this follows it. The microphone and the line in share one pair of inputs,
// so one of them is switched on at a time, and neither while the app listens nowhere. The microphone is one
// sound, so both sides are handed the same. Sound from the USB socket is not listened to here yet: an app that
// chooses it hears silence.
//
// Whoever makes the app's sound asks for one stretch of what came in before each stretch it makes. The card
// hands that over at the pace it plays, so the two stay in step. A card that hands nothing over for a while is
// no longer listened to, and the app goes on with silence at its inputs: its own sound is never held up.

#pragma once

#include "mp_forge_host_sound.h"
#include "mpc_sample_app.h"

#include <algorithm>
#include <atomic>
#include <cstdio>
#include <cstring>
#include <fcntl.h>
#include <string>
#include <sys/stat.h>
#include <unistd.h>
#include <vector>

#if defined(__linux__)
#include <poll.h>
#endif

namespace mp_forge_host {

#if defined(__linux__)

// The sound card, opened and made ready to be read from.
class ListeningCard {
public:
    enum class Heard { AStretch, NothingThisTime, NothingAnyMore };

    bool open(int file, int framesAStretch) {
        AskedOfTheCard hardware(framesAStretch, kStretchesWaiting);
        if (ioctl(file, SNDRV_PCM_IOCTL_HW_PARAMS, hardware.asked()) < 0) return false;
        snd_pcm_sw_params software{};
        software.tstamp_mode = SNDRV_PCM_TSTAMP_NONE;
        software.period_step = 1;
        software.avail_min = static_cast<snd_pcm_uframes_t>(framesAStretch);
        software.start_threshold = 1;
        software.stop_threshold = static_cast<snd_pcm_uframes_t>(framesAStretch) * kStretchesWaiting;
        return ioctl(file, SNDRV_PCM_IOCTL_SW_PARAMS, &software) >= 0 && begin(file);
    }

    // A card that had more come in than was taken is begun again, and that stretch is let go.
    Heard listen(int file, int32_t* frames, int frameCount) {
        pollfd watched{file, POLLIN, 0};
        if (poll(&watched, 1, kLongestWaitForAStretch) <= 0) return Heard::NothingAnyMore;
        snd_xferi taken{};
        taken.buf = frames;
        taken.frames = static_cast<snd_pcm_uframes_t>(frameCount);
        if (ioctl(file, SNDRV_PCM_IOCTL_READI_FRAMES, &taken) >= 0) return Heard::AStretch;
        const bool canBeBegunAgain = errno == EPIPE || errno == ESTRPIPE || errno == EINTR;
        return canBeBegunAgain && begin(file) ? Heard::NothingThisTime : Heard::NothingAnyMore;
    }

private:
    static constexpr int kStretchesWaiting = 4;
    // In thousandths of a second: many times what a stretch takes to come in.
    static constexpr int kLongestWaitForAStretch = 200;

    static bool begin(int file) { return ioctl(file, SNDRV_PCM_IOCTL_PREPARE) >= 0 && ioctl(file, SNDRV_PCM_IOCTL_START) >= 0; }
};

class Listening {
public:
    // `placeOfTheSound` is where the card plays: it is listened to at the same card. `mixer` is where the
    // card's switches are.
    Listening(const char* placeOfTheSound, const char* mixer) : place_(whereTheSameCardIsHeard(placeOfTheSound)), mixer_(mixer) {}
    ~Listening() { stopListening(); }

    // Said from anywhere: where the app now has its inputs listen.
    void listenAt(sample::SoundIn from) { asked_ = from; }

    // One stretch of what came in, for whoever makes the app's sound. Silence where nothing is listened to.
    void hear(float* left, float* right, int frames) {
        const sample::SoundIn asked = asked_.load();
        if (asked != listenedAt_) turnTo(asked, frames);
        if (file_ < 0 || !takeAStretch(frames)) return handOverSilence(left, right, frames);
        handOver(left, right, frames);
    }

private:
    static constexpr double kLoudest = 2147483648.0;
    static constexpr char kTheMicrophone[] = "Builtin Microphone";
    static constexpr char kTheLineIn[] = "TRS";

    // A sound card is played at a place whose name ends in p, and heard at the same name ending in c.
    static std::string whereTheSameCardIsHeard(const char* placeOfTheSound) {
        std::string place(placeOfTheSound);
        if (!place.empty() && place.back() == 'p') place.back() = 'c';
        return place;
    }

    static bool isHeardAtTheCard(sample::SoundIn from) { return from == sample::SoundIn::Mic || from == sample::SoundIn::LineIn; }

    void turnTo(sample::SoundIn from, int frames) {
        stopListening();
        listenedAt_ = from;
        if (from == sample::SoundIn::Usb) std::fprintf(stderr, "The app listens at USB, which is not listened to on the device yet. It hears silence.\n");
        if (isHeardAtTheCard(from)) beginListening(from, frames);
    }

    // The input that is not wanted is switched off before the other is switched on, so the two never meet.
    void beginListening(sample::SoundIn from, int frames) {
        const bool atTheMicrophone = from == sample::SoundIn::Mic;
        const char* wanted = atTheMicrophone ? kTheMicrophone : kTheLineIn;
        Mixer switches(mixer_.c_str());
        switches.turn(atTheMicrophone ? kTheLineIn : kTheMicrophone, false);
        const bool isSwitchedOn = switches.turn(wanted, true);
        const bool isOpen = isSwitchedOn && openTheCard(frames);
        std::fprintf(stderr, "Listening at %s: switching it on %s, and the sound card %s.\n", wanted, isSwitchedOn ? "worked" : "did not work",
                     isOpen ? "hands its sound over" : "does not hand its sound over");
    }

    bool openTheCard(int frames) {
        file_ = open(place_.c_str(), O_RDONLY | O_NONBLOCK | O_CLOEXEC);
        if (file_ >= 0 && isADevice() && waitsForItsSound() && card_.open(file_, frames)) return true;
        closeTheCard();
        return false;
    }

    bool isADevice() const {
        struct stat found {};
        return fstat(file_, &found) == 0 && S_ISCHR(found.st_mode);
    }

    // The card is opened without waiting, in case something else holds it, and read from with waiting.
    bool waitsForItsSound() const {
        const int flags = fcntl(file_, F_GETFL);
        return flags >= 0 && fcntl(file_, F_SETFL, flags & ~O_NONBLOCK) >= 0;
    }

    void stopListening() {
        closeTheCard();
        if (!isHeardAtTheCard(listenedAt_)) return;
        Mixer switches(mixer_.c_str());
        switches.turn(kTheMicrophone, false);
        switches.turn(kTheLineIn, false);
    }

    void closeTheCard() {
        if (file_ >= 0) close(file_);
        file_ = -1;
    }

    bool takeAStretch(int frames) {
        frames_.resize(static_cast<size_t>(frames) * kSides);
        const ListeningCard::Heard heard = card_.listen(file_, frames_.data(), frames);
        if (heard == ListeningCard::Heard::NothingAnyMore) giveUpListening();
        return heard == ListeningCard::Heard::AStretch;
    }

    void giveUpListening() {
        std::fprintf(stderr, "The sound card stopped handing over what comes in (%s). The app hears silence from here on.\n", std::strerror(errno));
        closeTheCard();
    }

    void handOver(float* left, float* right, int frames) const {
        for (int frame = 0; frame < frames; ++frame) {
            left[frame] = static_cast<float>(frames_[static_cast<size_t>(frame) * kSides] / kLoudest);
            right[frame] = static_cast<float>(frames_[static_cast<size_t>(frame) * kSides + 1] / kLoudest);
        }
        if (listenedAt_ == sample::SoundIn::Mic) makeBothSidesTheSame(left, right, frames);
    }

    static void makeBothSidesTheSame(float* left, float* right, int frames) {
        for (int frame = 0; frame < frames; ++frame) left[frame] = right[frame] = (left[frame] + right[frame]) * 0.5f;
    }

    static void handOverSilence(float* left, float* right, int frames) {
        std::fill(left, left + frames, 0.0f);
        std::fill(right, right + frames, 0.0f);
    }

    std::string place_;
    std::string mixer_;
    std::atomic<sample::SoundIn> asked_{sample::SoundIn::Off};
    sample::SoundIn listenedAt_ = sample::SoundIn::Off;
    int file_ = -1;
    ListeningCard card_;
    std::vector<int32_t> frames_;
};

#else

// Only the device has a sound card to listen at this way. Anywhere else an app run by this program hears silence.
class Listening {
public:
    Listening(const char* /*placeOfTheSound*/, const char* /*mixer*/) {}
    void listenAt(sample::SoundIn /*from*/) {}

    void hear(float* left, float* right, int frames) {
        std::fill(left, left + frames, 0.0f);
        std::fill(right, right + frames, 0.0f);
    }
};

#endif

}  // namespace mp_forge_host
