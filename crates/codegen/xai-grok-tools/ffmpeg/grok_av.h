/*
 * grok_av.h — Grok-owned narrow C ABI for in-process FFmpeg 8 media
 * preprocessing (plan section 10).
 *
 * Design invariants:
 *
 * 1. The Grok binary has NO link-time FFmpeg dependency. This shim calls
 *    FFmpeg exclusively through an immutable per-context function-pointer
 *    table (`GrokAvFns`) supplied by Rust via `libloading`. C never links
 *    against any `libav*` symbol directly.
 *
 * 2. C accepts NO paths. Input arrives as bounded in-memory bytes consumed
 *    through custom AVIO callbacks; Rust performs permission checks, path
 *    canonicalization, and bounded reads first.
 *
 * 3. Outputs are context-owned, allocated with `av_malloc`, and copied
 *    immediately into Rust, then released with `grok_av_frame_free` /
 *    `grok_av_pcm_free`.
 *
 * 4. Cancellation is a C-owned C11 atomic flag. `grok_av_cancel` is
 *    callable from any thread while the context is alive and only touches
 *    the atomic; decode loops and the format-level interrupt callback check
 *    it cooperatively.
 *
 * 5. Stuck contexts are never freed or reused: the Rust worker stops
 *    awaiting a hung native call, leaves the context allocated, and the
 *    owning thread/context may remain until process exit (plan 10.5).
 *
 * 6. Hard caps (bytes, pixels, dimensions, frames, duration, PCM samples)
 *    are enforced inside the shim so even hostile media cannot exceed
 *    bounded allocations.
 *
 * Compile with `-std=c11` against FFmpeg 8 headers (avutil 60, avcodec 62,
 * avformat 62, swscale 9, swresample 6).
 */
#ifndef GROK_AV_H
#define GROK_AV_H

#include <stdatomic.h>
#include <stddef.h>
#include <stdint.h>

#include <libavutil/avutil.h>
#include <libavutil/channel_layout.h>
#include <libavutil/error.h>
#include <libavutil/frame.h>
#include <libavutil/mathematics.h>
#include <libavutil/mem.h>
#include <libavutil/pixfmt.h>
#include <libavutil/rational.h>
#include <libavutil/samplefmt.h>
#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libswscale/swscale.h>
#include <libswresample/swresample.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ------------------------------------------------------------------ */
/* Error codes                                                         */
/* ------------------------------------------------------------------ */

typedef enum GrokAvError {
    GROK_AV_OK = 0,
    GROK_AV_ERR_NOMEM = 1,          /* allocation failed                       */
    GROK_AV_ERR_INVALID_ARG = 2,    /* bad argument                            */
    GROK_AV_ERR_LIBRARY = 3,        /* underlying FFmpeg call failed           */
    GROK_AV_ERR_OPEN = 4,           /* container open/probe failed             */
    GROK_AV_ERR_NO_STREAM = 5,      /* no matching stream                       */
    GROK_AV_ERR_DECODE = 6,         /* decode / conversion failed              */
    GROK_AV_ERR_EOF = 7,            /* end of media                            */
    GROK_AV_ERR_SEEK = 8,           /* seek failed                             */
    GROK_AV_ERR_CANCELLED = 9,      /* cooperative cancellation observed       */
    GROK_AV_ERR_UNSUPPORTED = 10,   /* format/codec/layout unsupported         */
    GROK_AV_ERR_LIMIT = 11          /* a configured safety cap was exceeded    */
} GrokAvError;

/* ------------------------------------------------------------------ */
/* Immutable per-context FFmpeg function-pointer table                 */
/* ------------------------------------------------------------------ */

/*
 * Supplied by Rust at open time and copied into the context. Field order,
 * sizes, and types MUST match the Rust `GrokAvFns` mirror exactly.
 */
typedef struct GrokAvFns {
    /* libavutil */
    unsigned (*avutil_version)(void);
    int (*av_strerror)(int errnum, char *errbuf, size_t errbuf_size);
    void *(*av_malloc)(size_t size);
    void *(*av_realloc)(void *ptr, size_t size);
    void (*av_free)(void *ptr);
    AVFrame *(*av_frame_alloc)(void);
    void (*av_frame_free)(AVFrame **frame);
    void (*av_frame_unref)(AVFrame *frame);
    int (*av_frame_get_buffer)(AVFrame *frame, int align);
    /* AVPacket helpers are exported by libavcodec (avcodec.h), not
     * libavutil; the loader resolves them from libavcodec. Field position
     * here is fixed by the Rust mirror and must not change. */
    AVPacket *(*av_packet_alloc)(void);
    void (*av_packet_free)(AVPacket **pkt);
    void (*av_packet_unref)(AVPacket *pkt);
    int (*av_samples_get_buffer_size)(int *linesize, int nb_channels,
                                      int nb_samples,
                                      enum AVSampleFormat sample_fmt,
                                      int align);
    int (*av_get_bytes_per_sample)(enum AVSampleFormat sample_fmt);
    void (*av_channel_layout_default)(AVChannelLayout *ch_layout,
                                      int nb_channels);
    void (*av_channel_layout_uninit)(AVChannelLayout *ch_layout);
    int64_t (*av_rescale_q)(int64_t a, AVRational bq, AVRational cq);

    /* libavcodec */
    unsigned (*avcodec_version)(void);
    AVCodecContext *(*avcodec_alloc_context3)(const AVCodec *codec);
    void (*avcodec_free_context)(AVCodecContext **avctx);
    int (*avcodec_parameters_to_context)(AVCodecContext *codec_ctx,
                                         const AVCodecParameters *par);
    int (*avcodec_open2)(AVCodecContext *avctx, const AVCodec *codec,
                         AVDictionary **options);
    int (*avcodec_send_packet)(AVCodecContext *avctx, const AVPacket *avpkt);
    int (*avcodec_receive_frame)(AVCodecContext *avctx, AVFrame *frame);
    void (*avcodec_flush_buffers)(AVCodecContext *avctx);

    /* libavformat */
    unsigned (*avformat_version)(void);
    AVFormatContext *(*avformat_alloc_context)(void);
    void (*avformat_free_context)(AVFormatContext *s);
    int (*avformat_open_input)(AVFormatContext **ps, const char *url,
                               const AVInputFormat *fmt,
                               AVDictionary **options);
    int (*avformat_find_stream_info)(AVFormatContext *ic,
                                     AVDictionary **options);
    int (*av_find_best_stream)(AVFormatContext *ic, enum AVMediaType type,
                               int wanted_stream_nb, int related_stream,
                               const AVCodec **decoder_ret, int flags);
    int (*av_read_frame)(AVFormatContext *s, AVPacket *pkt);
    int (*avformat_seek_file)(AVFormatContext *s, int stream_index,
                              int64_t min_ts, int64_t ts, int64_t max_ts,
                              int flags);
    AVIOContext *(*avio_alloc_context)(
        unsigned char *buffer, int buffer_size, int write_flag, void *opaque,
        int (*read_packet)(void *opaque, uint8_t *buf, int buf_size),
        int (*write_packet)(void *opaque, const uint8_t *buf, int buf_size),
        int64_t (*seek)(void *opaque, int64_t offset, int whence));
    void (*avio_context_free)(AVIOContext **s);

    /* libswscale */
    unsigned (*swscale_version)(void);
    struct SwsContext *(*sws_getContext)(
        int srcW, int srcH, enum AVPixelFormat srcFormat, int dstW, int dstH,
        enum AVPixelFormat dstFormat, int flags, SwsFilter *srcFilter,
        SwsFilter *dstFilter, const double *param);
    int (*sws_scale)(struct SwsContext *c, const uint8_t *const srcSlice[],
                     const int srcStride[], int srcSliceY, int srcSliceH,
                     uint8_t *const dst[], const int dstStride[]);
    void (*sws_freeContext)(struct SwsContext *swsContext);

    /* libswresample */
    unsigned (*swresample_version)(void);
    struct SwrContext *(*swr_alloc)(void);
    int (*swr_init)(struct SwrContext *s);
    void (*swr_free)(struct SwrContext **s);
    int (*swr_alloc_set_opts2)(
        struct SwrContext **ps, const AVChannelLayout *out_ch_layout,
        enum AVSampleFormat out_sample_fmt, int out_sample_rate,
        const AVChannelLayout *in_ch_layout, enum AVSampleFormat in_sample_fmt,
        int in_sample_rate, int log_offset, void *log_ctx);
    int (*swr_convert)(struct SwrContext *s, uint8_t **out, int out_count,
                       const uint8_t **in, int in_count);
    int (*swr_get_out_samples)(struct SwrContext *s, int in_samples);
} GrokAvFns;

/* ------------------------------------------------------------------ */
/* Hard limits and output structs                                      */
/* ------------------------------------------------------------------ */

typedef struct GrokAvLimits {
    size_t max_source_bytes;   /* input byte cap (Rust enforces too)      */
    uint64_t max_pixels;       /* decoded frame width*height cap          */
    int max_width;             /* decoded frame width cap                 */
    int max_height;            /* decoded frame height cap                */
    uint64_t max_duration_us;  /* media duration cap in microseconds (enforced at open) */
    uint64_t max_audio_samples; /* bound on total output PCM samples       */
    int max_video_frames;      /* bound on sequential frame iterations    */
    uint64_t max_frame_bytes;  /* per-frame RGB output cap                */
} GrokAvLimits;

typedef struct GrokAvProbeResult {
    int has_video;
    int has_audio;
    int width;               /* video */
    int height;              /* video */
    int video_stream_index;
    int audio_stream_index;
    int64_t duration_us;     /* AV_TIME_BASE units (microseconds) */
    int64_t start_time_us;   /* AV_TIME_BASE units */
    int64_t video_time_base_num;
    int64_t video_time_base_den;
    int sample_rate;         /* audio */
    int channels;            /* audio */
} GrokAvProbeResult;

typedef struct GrokAvFrame {
    uint8_t *data;           /* RGB24 packed, row-major, av_malloc'd       */
    int width;
    int height;
    int stride;              /* bytes per row (may exceed width*3)          */
    int64_t pts;             /* stream time-base units; AV_NOPTS if unknown */
    int64_t time_base_num;
    int64_t time_base_den;
} GrokAvFrame;

typedef struct GrokAvPcm {
    float *data;             /* interleaved float32, normalized to [-1,1]  */
    size_t len;              /* number of samples (frames * channels)       */
    int sample_rate;
    int channels;
    int truncated;           /* 1 when the audio cap stopped output early   */
} GrokAvPcm;

/* Opaque context. Created by grok_av_open, destroyed by grok_av_close. */
typedef struct GrokAvContext GrokAvContext;

/* ------------------------------------------------------------------ */
/* Entry points (compiled into libgrok_av.a, called from Rust)          */
/* ------------------------------------------------------------------ */

/*
 * Open and probe a container from bounded in-memory bytes.
 *
 * Media whose probed duration strictly exceeds `limits->max_duration_us`
 * is rejected with GROK_AV_ERR_LIMIT; unknown durations pass.
 *
 * On ANY return value (success or error) `*out_ctx` is set and MUST be
 * released with grok_av_close. On error, grok_av_last_error describes why.
 */
int grok_av_open(GrokAvContext **out_ctx, const GrokAvFns *fns,
                 const uint8_t *data, size_t len, const GrokAvLimits *limits);

int grok_av_probe(GrokAvContext *ctx, GrokAvProbeResult *out);

/* Decode the next video frame to RGB24, or GROK_AV_ERR_EOF. */
int grok_av_next_frame(GrokAvContext *ctx, GrokAvFrame *out);

/* Deterministic frame at (or just after) a given whole second. */
int grok_av_frame_at_seconds(GrokAvContext *ctx, int64_t seconds,
                             GrokAvFrame *out);

/* Decode the whole audio stream to bounded normalized interleaved float32. */
int grok_av_audio_pcm(GrokAvContext *ctx, GrokAvPcm *out);

/* Thread-safe cooperative cancellation; only touches the C11 atomic flag. */
int grok_av_cancel(GrokAvContext *ctx);

/* Last error message, owned by ctx, valid until the next operation. */
const char *grok_av_last_error(GrokAvContext *ctx);

/* Release outputs copied into Rust. */
void grok_av_frame_free(GrokAvFrame *frame);
void grok_av_pcm_free(GrokAvPcm *pcm);

/* Destroy a context. Only call after the owning worker has finished. */
void grok_av_close(GrokAvContext *ctx);

#ifdef __cplusplus
}
#endif

#endif /* GROK_AV_H */
