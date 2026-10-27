#include <signalsmith-stretch.h>

#include <cstddef>
#include <cstdlib>
#include <new>

#ifdef FOURFOUR_COUNT_ALLOCS
namespace {
thread_local bool counting = false;
thread_local size_t allocs = 0;
thread_local size_t frees = 0;
}

void *operator new(size_t size) {
    if (counting) ++allocs;
    void *p = std::malloc(size ? size : 1);
    if (!p) throw std::bad_alloc();
    return p;
}

void operator delete(void *p) noexcept {
    if (counting && p) ++frees;
    std::free(p);
}

void operator delete(void *p, size_t) noexcept {
    if (counting && p) ++frees;
    std::free(p);
}

extern "C" void ff_stretch_count_arm(int on) {
    counting = on != 0;
    if (on) allocs = frees = 0;
}

extern "C" void ff_stretch_count_read(size_t *a, size_t *f) {
    *a = allocs;
    *f = frees;
}
#else
extern "C" void ff_stretch_count_arm(int) {}
extern "C" void ff_stretch_count_read(size_t *a, size_t *f) {
    *a = 0;
    *f = 0;
}
#endif

namespace {

class InterleavedBuffer {
    float *data;
    size_t channelCount;

public:
    InterleavedBuffer(float *data, size_t channels) : data(data), channelCount(channels) {}

    struct ChannelView {
        float *data;
        size_t channel;
        size_t stride;
        float &operator[](size_t offset) const { return data[offset * stride + channel]; }
    };

    ChannelView operator[](size_t channel) const { return {data, channel, channelCount}; }
};

}

struct ff_stretch {
    signalsmith::stretch::SignalsmithStretch<float> instance;
    size_t channels;

    explicit ff_stretch(long seed) : instance(seed) {}
};

extern "C" {

ff_stretch *ff_stretch_create(int channels, int block, int interval, int split_computation, long seed) {
    auto *h = new ff_stretch(seed);
    h->channels = static_cast<size_t>(channels);
    h->instance.configure(channels, block, interval, split_computation != 0);
    return h;
}

void ff_stretch_destroy(ff_stretch *h) { delete h; }

void ff_stretch_reset(ff_stretch *h) { h->instance.reset(); }

int ff_stretch_block_samples(const ff_stretch *h) { return h->instance.blockSamples(); }

int ff_stretch_interval_samples(const ff_stretch *h) { return h->instance.intervalSamples(); }

int ff_stretch_input_latency(const ff_stretch *h) { return h->instance.inputLatency(); }

int ff_stretch_output_latency(const ff_stretch *h) { return h->instance.outputLatency(); }

void ff_stretch_set_transpose_factor(ff_stretch *h, float factor) {
    h->instance.setTransposeFactor(factor, 0.0f);
}

void ff_stretch_seek(ff_stretch *h, float *input, int frames, double playback_rate) {
    h->instance.seek(InterleavedBuffer(input, h->channels), frames, playback_rate);
}

void ff_stretch_process(ff_stretch *h, float *input, int in_frames, float *output, int out_frames) {
    h->instance.process(InterleavedBuffer(input, h->channels), in_frames,
                        InterleavedBuffer(output, h->channels), out_frames);
}

}
