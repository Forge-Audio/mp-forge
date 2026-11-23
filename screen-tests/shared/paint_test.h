// The paint test of the "Screen Probe" test card: does a block painted into the screen's file show, and stay?
//
// SPDX-License-Identifier: MIT
//
// The test takes eight seconds, in two halves, and is moved on one step at a time by whoever runs it:
//
//   1. PAINTED ONCE, three seconds. A magenta block is painted one time. At every step the test looks whether
//      the block is still in the file. An MPC that paints its whole picture over and over wipes it at once.
//   2. PAINTED OVER, five seconds. A green block is painted again at every step, after looking whether the
//      one before was still there. This is how a picture would be kept up against an MPC that paints over it.
//
// The block is wider than it is tall and has a white mark in the corner the file calls top-left, so a photo of
// the screen shows whether the screen is turned or mirrored against its file, and where the file's dots land.
// What was under the block is put back at the end.

#pragma once

#include "screen_file.h"

namespace screen_test {

class PaintTest
{
public:
    enum Stage { kNotYet = 0, kCannotHere, kCouldNotWrite, kPaintedOnce, kPaintingOver, kDone };

    static constexpr int   kStepMs = 20;
    static constexpr Block kBlock { 400, 300, 300, 150 };
    static constexpr int   kMarkSize = 40;

    explicit PaintTest(const ScreenFile& screen) : screen_(screen) {}

    Stage stage() const { return stage_; }
    bool  isUnderWay() const { return stage_ == kPaintedOnce || stage_ == kPaintingOver; }
    int   percentKeptWhenPaintedOnce() const { return once_.percentKept(); }
    int   percentKeptWhenPaintedOver() const { return over_.percentKept(); }

    void begin()
    {
        once_ = over_ = Looks {};
        if (!screen_.holds(kBlock) || !screen_.read(kBlock, underneath_))
            stage_ = kCannotHere;
        else
            startStage(kPaintedOnce, kMagenta);
    }

    void step()
    {
        if (stage_ == kPaintedOnce)
            stepPaintedOnce();
        else if (stage_ == kPaintingOver)
            stepPaintingOver();
    }

    // Puts back what was under the block when the test is cut short.
    void endEarly()
    {
        if (isUnderWay())
            finish();
    }

private:
    struct Looks
    {
        int made = 0, kept = 0;

        void add(bool wasKept)
        {
            ++made;
            kept += wasKept ? 1 : 0;
        }

        int percentKept() const { return made == 0 ? 0 : kept * 100 / made; }
    };

    static constexpr int    kStepsPaintedOnce  = 3000 / kStepMs;
    static constexpr int    kStepsPaintingOver = 5000 / kStepMs;
    static constexpr Colour kMagenta { 255, 0, 255 };
    static constexpr Colour kGreen { 0, 255, 0 };
    static constexpr Colour kWhite { 255, 255, 255 };

    void stepPaintedOnce()
    {
        once_.add(isStillThere());
        if (++steps_ >= kStepsPaintedOnce)
            startStage(kPaintingOver, kGreen);
    }

    void stepPaintingOver()
    {
        over_.add(isStillThere());
        if (++steps_ >= kStepsPaintingOver)
            finish();
        else if (!screen_.write(kBlock, painted_))
            stage_ = kCouldNotWrite;
    }

    void startStage(Stage stage, Colour colour)
    {
        painted_ = blockIn(colour);
        steps_   = 0;
        stage_   = screen_.write(kBlock, painted_) ? stage : kCouldNotWrite;
    }

    void finish()
    {
        stage_ = screen_.write(kBlock, underneath_) ? kDone : kCouldNotWrite;
    }

    bool isStillThere()
    {
        return screen_.read(kBlock, found_) && found_ == painted_;
    }

    std::vector<uint8_t> blockIn(Colour colour) const
    {
        const ScreenLayout&  layout = screen_.layout();
        std::vector<uint8_t> dots(layout.bytesOf(kBlock));
        for (int row = 0; row < kBlock.down; ++row)
            for (int column = 0; column < kBlock.across; ++column)
            {
                const bool     isOnTheMark = row < kMarkSize && column < kMarkSize;
                const uint32_t dot         = layout.dotOf(isOnTheMark ? kWhite : colour);
                std::memcpy(&dots[((size_t)row * kBlock.across + column) * layout.bytesADot], &dot, (size_t)layout.bytesADot);
            }
        return dots;
    }

    const ScreenFile&    screen_;
    Stage                stage_ = kNotYet;
    int                  steps_ = 0;
    Looks                once_, over_;
    std::vector<uint8_t> underneath_, painted_, found_;
};

} // namespace screen_test
