/*
 * grok_av.c — Grok-owned narrow C shim over FFmpeg 8 (plan section 10.4).
 *
 * FFmpeg is called ONLY through the per-context `GrokAvFns` table supplied
 * by Rust, so this object file has no undefined FFmpeg symbols and the Grok
 * binary never links FFmpeg.
 *
 * Input is a bounded byte slice consumed through custom AVIO callbacks;
 * there are no paths. Outputs are context-owned and copied into Rust by the
 * caller. Cancellation is a C11 atomic flag owned by C.
 */
#include "grok_av.h"

#include <errno.h>
#include <inttypes.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/*
 * Every output buffer is preceded by a small header that records the
 * `av_free` of the library that allocated it, so `grok_av_frame_free` /
 * `grok_av_pcm_free` always release through the correct allocator without
 * keeping a back-pointer to the context.
 */
typedef struct GrokAvBufHdr {
    void (*free_fn)(void *);
} GrokAvBufHdr;

static void grok_av_buf_free(uint8_t *payload) {
    if (payload == NULL)
        return;
    GrokAvBufHdr *hdr = (GrokAvBufHdr *)payload - 1;
    if (hdr->free_fn != NULL)
        hdr->free_fn((void *)hdr);
}

static uint8_t *grok_av_buf_alloc(const GrokAvFns *F, size_t payload) {
    size_t total = sizeof(GrokAvBufHdr) + payload;
    GrokAvBufHdr *hdr = (GrokAvBufHdr *)F->av_malloc(total);
    if (hdr == NULL)
        return NULL;
    hdr->free_fn = F->av_free;
    return (uint8_t *)(hdr + 1);
}

struct GrokAvContext {
    GrokAvFns fns;            /* immutable copy of the function table      */
    GrokAvLimits limits;      /* immutable copy of the hard caps           */

    const uint8_t *data;      /* borrowed input bytes (owned by Rust)      */
    size_t len;
    size_t pos;

    _Atomic int cancel_flag;  /* C-owned C11 atomic; never Rust layout      */

    char errbuf[256];

    AVFormatContext *fmt;
    AVIOContext *avio;
    uint8_t *avio_buf;

    int video_stream_index;
    int audio_stream_index;
    const AVCodec *video_codec; /* from av_find_best_stream, may be NULL    */
    const AVCodec *audio_codec;

    AVCodecContext *video_dec;  /* lazily opened                            */
    AVCodecContext *audio_dec;
    struct SwsContext *sws;     /* cached scaler for a fixed size           */
    struct SwrContext *swr;     /* lazy resampler for audio                 */
    AVFrame *rgb_frame;         /* reusable RGB24 output frame              */
    int rgb_width;
    int rgb_height;
    int frame_count;            /* sequential iteration counter             */
};

/* ------------------------------------------------------------------ */
/* Internal helpers                                                     */
/* ------------------------------------------------------------------ */

static int grok_av_is_cancelled(GrokAvContext *ctx) {
    return atomic_load_explicit(&ctx->cancel_flag, memory_order_relaxed) != 0;
}

static const char *grok_av_error_name(int code) {
    switch (code) {
    case GROK_AV_OK: return "ok";
    case GROK_AV_ERR_NOMEM: return "out of memory";
    case GROK_AV_ERR_INVALID_ARG: return "invalid argument";
    case GROK_AV_ERR_LIBRARY: return "library error";
    case GROK_AV_ERR_OPEN: return "open failed";
    case GROK_AV_ERR_NO_STREAM: return "no stream";
    case GROK_AV_ERR_DECODE: return "decode failed";
    case GROK_AV_ERR_EOF: return "end of media";
    case GROK_AV_ERR_SEEK: return "seek failed";
    case GROK_AV_ERR_CANCELLED: return "cancelled";
    case GROK_AV_ERR_UNSUPPORTED: return "unsupported";
    case GROK_AV_ERR_LIMIT: return "limit exceeded";
    default: return "unknown";
    }
}

static void grok_av_fail(GrokAvContext *ctx, int code, int ff_errnum) {
    const GrokAvFns *F = &ctx->fns;
    if (ff_errnum != 0 && F->av_strerror != NULL) {
        char buf[128];
        F->av_strerror(ff_errnum, buf, sizeof(buf));
        snprintf(ctx->errbuf, sizeof(ctx->errbuf), "ffmpeg error %d: %s",
                 ff_errnum, buf);
    } else if (ff_errnum != 0) {
        snprintf(ctx->errbuf, sizeof(ctx->errbuf), "ffmpeg error %d",
                 ff_errnum);
    } else {
        snprintf(ctx->errbuf, sizeof(ctx->errbuf), "%s",
                 grok_av_error_name(code));
    }
}

static int grok_av_no_stream(GrokAvContext *ctx, const char *which) {
    snprintf(ctx->errbuf, sizeof(ctx->errbuf), "no %s stream", which);
    return GROK_AV_ERR_NO_STREAM;
}

/* ------------------------------------------------------------------ */
/* Custom AVIO callbacks (pure memory, no paths)                        */
/* ------------------------------------------------------------------ */

static int grok_av_io_read(void *opaque, uint8_t *buf, int buf_size) {
    GrokAvContext *ctx = (GrokAvContext *)opaque;
    if (grok_av_is_cancelled(ctx))
        return AVERROR(EINTR);
    if (ctx->pos >= ctx->len)
        /* End of input: FFmpeg read callbacks must return AVERROR_EOF here,
         * never 0. `avio_read`'s bypass path (size > buffer_size) loops
         * forever when the callback returns 0, because `size` never
         * decreases (avio_read in libavformat/aviobuf.c). */
        return AVERROR_EOF;
    size_t avail = ctx->len - ctx->pos;
    int to_read = (size_t)buf_size < avail ? buf_size : (int)avail;
    memcpy(buf, ctx->data + ctx->pos, (size_t)to_read);
    ctx->pos += (size_t)to_read;
    return to_read;
}

static int64_t grok_av_io_seek(void *opaque, int64_t offset, int whence) {
    GrokAvContext *ctx = (GrokAvContext *)opaque;
    if (whence == AVSEEK_SIZE)
        return (int64_t)ctx->len;
    int64_t new_pos;
    switch (whence) {
    case SEEK_SET:
        new_pos = offset;
        break;
    case SEEK_CUR:
        new_pos = (int64_t)ctx->pos + offset;
        break;
    case SEEK_END:
        new_pos = (int64_t)ctx->len + offset;
        break;
    default:
        return AVERROR(EINVAL);
    }
    if (new_pos < 0)
        return AVERROR(EINVAL);
    ctx->pos = (size_t)new_pos;
    return new_pos;
}

static int grok_av_interrupt(void *opaque) {
    GrokAvContext *ctx = (GrokAvContext *)opaque;
    return grok_av_is_cancelled(ctx) ? 1 : 0;
}

/* ------------------------------------------------------------------ */
/* Stream/decoder setup                                                 */
/* ------------------------------------------------------------------ */

static int grok_av_prepare_video_decoder(GrokAvContext *ctx) {
    const GrokAvFns *F = &ctx->fns;
    if (ctx->video_stream_index < 0)
        return grok_av_no_stream(ctx, "video");
    if (ctx->video_dec != NULL)
        return GROK_AV_OK;
    const AVCodec *codec = ctx->video_codec;
    AVStream *st = ctx->fmt->streams[ctx->video_stream_index];
    ctx->video_dec = F->avcodec_alloc_context3(codec);
    if (ctx->video_dec == NULL)
        return GROK_AV_ERR_NOMEM;
    int ret = F->avcodec_parameters_to_context(ctx->video_dec, st->codecpar);
    if (ret < 0) {
        grok_av_fail(ctx, GROK_AV_ERR_DECODE, ret);
        return GROK_AV_ERR_DECODE;
    }
    ret = F->avcodec_open2(ctx->video_dec, codec, NULL);
    if (ret < 0) {
        grok_av_fail(ctx, GROK_AV_ERR_DECODE, ret);
        return GROK_AV_ERR_DECODE;
    }
    return GROK_AV_OK;
}

static int grok_av_prepare_audio_decoder(GrokAvContext *ctx) {
    const GrokAvFns *F = &ctx->fns;
    if (ctx->audio_stream_index < 0)
        return grok_av_no_stream(ctx, "audio");
    if (ctx->audio_dec != NULL)
        return GROK_AV_OK;
    const AVCodec *codec = ctx->audio_codec;
    AVStream *st = ctx->fmt->streams[ctx->audio_stream_index];
    ctx->audio_dec = F->avcodec_alloc_context3(codec);
    if (ctx->audio_dec == NULL)
        return GROK_AV_ERR_NOMEM;
    int ret = F->avcodec_parameters_to_context(ctx->audio_dec, st->codecpar);
    if (ret < 0) {
        grok_av_fail(ctx, GROK_AV_ERR_DECODE, ret);
        return GROK_AV_ERR_DECODE;
    }
    ret = F->avcodec_open2(ctx->audio_dec, codec, NULL);
    if (ret < 0) {
        grok_av_fail(ctx, GROK_AV_ERR_DECODE, ret);
        return GROK_AV_ERR_DECODE;
    }
    return GROK_AV_OK;
}

/* ------------------------------------------------------------------ */
/* Video frame decode + RGB24 output                                    */
/* ------------------------------------------------------------------ */

static int grok_av_output_rgb(GrokAvContext *ctx, AVFrame *frame,
                              GrokAvFrame *out) {
    const GrokAvFns *F = &ctx->fns;
    if (frame == NULL || frame->width <= 0 || frame->height <= 0) {
        grok_av_fail(ctx, GROK_AV_ERR_DECODE, 0);
        return GROK_AV_ERR_DECODE;
    }
    uint64_t pixels = (uint64_t)frame->width * (uint64_t)frame->height;
    uint64_t frame_bytes = pixels * 3;
    if (ctx->limits.max_pixels > 0 && pixels > ctx->limits.max_pixels) {
        grok_av_fail(ctx, GROK_AV_ERR_LIMIT, 0);
        return GROK_AV_ERR_LIMIT;
    }
    if (ctx->limits.max_width > 0 && frame->width > ctx->limits.max_width) {
        grok_av_fail(ctx, GROK_AV_ERR_LIMIT, 0);
        return GROK_AV_ERR_LIMIT;
    }
    if (ctx->limits.max_height > 0 && frame->height > ctx->limits.max_height) {
        grok_av_fail(ctx, GROK_AV_ERR_LIMIT, 0);
        return GROK_AV_ERR_LIMIT;
    }
    if (ctx->limits.max_frame_bytes > 0 &&
        frame_bytes > ctx->limits.max_frame_bytes) {
        grok_av_fail(ctx, GROK_AV_ERR_LIMIT, 0);
        return GROK_AV_ERR_LIMIT;
    }

    if (ctx->sws == NULL || ctx->rgb_frame == NULL ||
        ctx->rgb_width != frame->width || ctx->rgb_height != frame->height) {
        if (ctx->sws != NULL) {
            F->sws_freeContext(ctx->sws);
            ctx->sws = NULL;
        }
        if (ctx->rgb_frame != NULL) {
            F->av_frame_free(&ctx->rgb_frame);
            ctx->rgb_frame = NULL;
        }
        ctx->sws = F->sws_getContext(
            frame->width, frame->height, (enum AVPixelFormat)frame->format,
            frame->width, frame->height, AV_PIX_FMT_RGB24, SWS_BILINEAR, NULL,
            NULL, NULL);
        if (ctx->sws == NULL) {
            grok_av_fail(ctx, GROK_AV_ERR_DECODE, 0);
            return GROK_AV_ERR_DECODE;
        }
        ctx->rgb_frame = F->av_frame_alloc();
        if (ctx->rgb_frame == NULL)
            return GROK_AV_ERR_NOMEM;
        ctx->rgb_frame->format = AV_PIX_FMT_RGB24;
        ctx->rgb_frame->width = frame->width;
        ctx->rgb_frame->height = frame->height;
        int ret = F->av_frame_get_buffer(ctx->rgb_frame, 32);
        if (ret < 0) {
            grok_av_fail(ctx, GROK_AV_ERR_DECODE, ret);
            return GROK_AV_ERR_DECODE;
        }
        ctx->rgb_width = frame->width;
        ctx->rgb_height = frame->height;
    }

    F->sws_scale(ctx->sws, (const uint8_t *const *)frame->data,
                 frame->linesize, 0, frame->height, ctx->rgb_frame->data,
                 ctx->rgb_frame->linesize);

    int stride = ctx->rgb_frame->linesize[0];
    size_t total = (size_t)ctx->rgb_height * (size_t)stride;
    uint8_t *dst = grok_av_buf_alloc(F, total);
    if (dst == NULL)
        return GROK_AV_ERR_NOMEM;
    for (int y = 0; y < ctx->rgb_height; y++) {
        memcpy(dst + (size_t)y * (size_t)stride,
               ctx->rgb_frame->data[0] + (size_t)y * (size_t)stride,
               (size_t)stride);
    }
    out->data = dst;
    out->width = ctx->rgb_width;
    out->height = ctx->rgb_height;
    out->stride = stride;
    out->pts = frame->pts;
    if (ctx->video_stream_index >= 0) {
        AVStream *st = ctx->fmt->streams[ctx->video_stream_index];
        out->time_base_num = st->time_base.num;
        out->time_base_den = st->time_base.den;
    } else {
        out->time_base_num = 0;
        out->time_base_den = 0;
    }
    return GROK_AV_OK;
}

/*
 * Shared video decode loop. If `min_pts` is non-NULL, decode forward until a
 * frame with pts >= *min_pts (used by frame_at_seconds); otherwise return
 * the next available frame. Bounded by max_video_frames and cancellation.
 *
 * Return value contract: GROK_AV_OK exactly when a frame was written to
 * `out`; GROK_AV_ERR_EOF when the media ended without a frame; other error
 * codes for decode/cancellation/limit failures.
 */
static int grok_av_decode_video(GrokAvContext *ctx, const int64_t *min_pts,
                                GrokAvFrame *out) {
    const GrokAvFns *F = &ctx->fns;
    int ret = grok_av_prepare_video_decoder(ctx);
    if (ret != GROK_AV_OK)
        return ret;

    if (ctx->limits.max_video_frames > 0 &&
        ctx->frame_count >= ctx->limits.max_video_frames) {
        grok_av_fail(ctx, GROK_AV_ERR_LIMIT, 0);
        return GROK_AV_ERR_LIMIT;
    }

    AVPacket *pkt = F->av_packet_alloc();
    if (pkt == NULL)
        return GROK_AV_ERR_NOMEM;
    AVFrame *frame = F->av_frame_alloc();
    if (frame == NULL) {
        F->av_packet_free(&pkt);
        return GROK_AV_ERR_NOMEM;
    }

    /*
     * Start as EOF; a successfully produced frame (below) transitions the
     * status to GROK_AV_OK so callers distinguish media end from a valid
     * frame.
     */
    int status = GROK_AV_ERR_EOF;
    for (;;) {
        if (grok_av_is_cancelled(ctx)) {
            status = GROK_AV_ERR_CANCELLED;
            break;
        }
        ret = F->av_read_frame(ctx->fmt, pkt);
        if (ret == AVERROR_EOF) {
            status = GROK_AV_ERR_EOF;
            break;
        }
        if (ret < 0) {
            if (grok_av_is_cancelled(ctx)) {
                status = GROK_AV_ERR_CANCELLED;
                break;
            }
            grok_av_fail(ctx, GROK_AV_ERR_DECODE, ret);
            status = GROK_AV_ERR_DECODE;
            break;
        }
        if (pkt->stream_index != ctx->video_stream_index) {
            F->av_packet_unref(pkt);
            continue;
        }
        ret = F->avcodec_send_packet(ctx->video_dec, pkt);
        F->av_packet_unref(pkt);
        if (ret < 0 && ret != AVERROR(EAGAIN)) {
            grok_av_fail(ctx, GROK_AV_ERR_DECODE, ret);
            status = GROK_AV_ERR_DECODE;
            break;
        }
        /* Drain frames from the decoder for this packet. */
        int got_frame = 0;
        for (;;) {
            if (grok_av_is_cancelled(ctx)) {
                status = GROK_AV_ERR_CANCELLED;
                break;
            }
            ret = F->avcodec_receive_frame(ctx->video_dec, frame);
            if (ret == AVERROR(EAGAIN))
                break;
            if (ret < 0) {
                if (ret == AVERROR_EOF)
                    break;
                grok_av_fail(ctx, GROK_AV_ERR_DECODE, ret);
                status = GROK_AV_ERR_DECODE;
                break;
            }
            if (min_pts != NULL && frame->pts != AV_NOPTS_VALUE &&
                frame->pts < *min_pts) {
                F->av_frame_unref(frame);
                continue;
            }
            ret = grok_av_output_rgb(ctx, frame, out);
            F->av_frame_unref(frame);
            if (ret != GROK_AV_OK) {
                status = ret;
                break;
            }
            ctx->frame_count++;
            got_frame = 1;
            status = GROK_AV_OK;
            break;
        }
        if (status != GROK_AV_ERR_EOF)
            break; /* OK / DECODE / LIMIT / CANCELLED */
        if (got_frame)
            break;
        /* No frame from this packet; read the next one. */
    }

    F->av_frame_free(&frame);
    F->av_packet_free(&pkt);
    return status;
}

/* ------------------------------------------------------------------ */
/* Audio PCM decoding                                                   */
/* ------------------------------------------------------------------ */

static int grok_av_prepare_swr(GrokAvContext *ctx) {
    const GrokAvFns *F = &ctx->fns;
    if (ctx->swr != NULL)
        return GROK_AV_OK;
    AVCodecParameters *par =
        ctx->fmt->streams[ctx->audio_stream_index]->codecpar;
    if (par->sample_rate <= 0 || par->ch_layout.nb_channels <= 0) {
        grok_av_fail(ctx, GROK_AV_ERR_UNSUPPORTED, 0);
        return GROK_AV_ERR_UNSUPPORTED;
    }
    AVChannelLayout out_layout;
    F->av_channel_layout_default(&out_layout, par->ch_layout.nb_channels);
    int ret = F->swr_alloc_set_opts2(
        &ctx->swr, &out_layout, AV_SAMPLE_FMT_FLT, par->sample_rate,
        &par->ch_layout, (enum AVSampleFormat)par->format, par->sample_rate, 0,
        NULL);
    F->av_channel_layout_uninit(&out_layout);
    if (ret < 0) {
        grok_av_fail(ctx, GROK_AV_ERR_DECODE, ret);
        return GROK_AV_ERR_DECODE;
    }
    ret = F->swr_init(ctx->swr);
    if (ret < 0) {
        grok_av_fail(ctx, GROK_AV_ERR_DECODE, ret);
        return GROK_AV_ERR_DECODE;
    }
    return GROK_AV_OK;
}

int grok_av_audio_pcm(GrokAvContext *ctx, GrokAvPcm *out) {
    const GrokAvFns *F = &ctx->fns;
    if (ctx == NULL || out == NULL)
        return GROK_AV_ERR_INVALID_ARG;
    memset(out, 0, sizeof(*out));
    if (ctx->audio_stream_index < 0)
        return grok_av_no_stream(ctx, "audio");
    int ret = grok_av_prepare_audio_decoder(ctx);
    if (ret != GROK_AV_OK)
        return ret;
    ret = grok_av_prepare_swr(ctx);
    if (ret != GROK_AV_OK)
        return ret;

    AVCodecParameters *par =
        ctx->fmt->streams[ctx->audio_stream_index]->codecpar;
    int channels = par->ch_layout.nb_channels;
    const size_t bytes_per_sample = sizeof(float);

    uint64_t max_samples = ctx->limits.max_audio_samples;
    if (max_samples == 0)
        max_samples = UINT64_MAX;

    /* Initial capacity: 4096 output frames. */
    int init_cap = F->av_samples_get_buffer_size(NULL, channels, 4096,
                                                 AV_SAMPLE_FMT_FLT, 1);
    if (init_cap <= 0) {
        grok_av_fail(ctx, GROK_AV_ERR_DECODE, init_cap);
        return GROK_AV_ERR_DECODE;
    }
    size_t capacity = (size_t)init_cap;
    uint8_t *buf = grok_av_buf_alloc(F, capacity);
    if (buf == NULL)
        return GROK_AV_ERR_NOMEM;
    size_t used = 0;
    uint64_t total_frames = 0; /* number of sample frames (per channel) */
    int truncated = 0;
    int status = GROK_AV_OK;

    AVPacket *pkt = F->av_packet_alloc();
    if (pkt == NULL) {
        grok_av_buf_free(buf);
        return GROK_AV_ERR_NOMEM;
    }
    AVFrame *frame = F->av_frame_alloc();
    if (frame == NULL) {
        F->av_packet_free(&pkt);
        grok_av_buf_free(buf);
        return GROK_AV_ERR_NOMEM;
    }

    int done = 0;
    while (!done) {
        if (grok_av_is_cancelled(ctx)) {
            status = GROK_AV_ERR_CANCELLED;
            break;
        }
        ret = F->av_read_frame(ctx->fmt, pkt);
        if (ret == AVERROR_EOF) {
            /* Flush the resampler. */
            int avail = F->swr_get_out_samples(ctx->swr, 0);
            if (avail > 0) {
                uint64_t remaining =
                    max_samples == UINT64_MAX ? UINT64_MAX
                                              : max_samples - total_frames;
                int out_count = remaining == UINT64_MAX
                                    ? avail
                                    : (remaining < (uint64_t)avail
                                           ? (int)remaining
                                           : avail);
                if (out_count > 0) {
                    size_t needed =
                        used + (size_t)out_count * (size_t)channels *
                                   bytes_per_sample;
                    if (needed > capacity) {
                        uint8_t *nb = grok_av_buf_alloc(F, needed);
                        if (nb == NULL) {
                            status = GROK_AV_ERR_NOMEM;
                            break;
                        }
                        memcpy(nb, buf, used);
                        grok_av_buf_free(buf);
                        buf = nb;
                        capacity = needed;
                    }
                    uint8_t *write_ptr = buf + used;
                    int converted =
                        F->swr_convert(ctx->swr, &write_ptr, out_count, NULL,
                                       0);
                    if (converted < 0) {
                        grok_av_fail(ctx, GROK_AV_ERR_DECODE, converted);
                        status = GROK_AV_ERR_DECODE;
                        break;
                    }
                    used += (size_t)converted * (size_t)channels *
                            bytes_per_sample;
                    total_frames += (uint64_t)converted;
                }
            }
            done = 1;
            break;
        }
        if (ret < 0) {
            if (grok_av_is_cancelled(ctx)) {
                status = GROK_AV_ERR_CANCELLED;
                break;
            }
            grok_av_fail(ctx, GROK_AV_ERR_DECODE, ret);
            status = GROK_AV_ERR_DECODE;
            break;
        }
        if (pkt->stream_index != ctx->audio_stream_index) {
            F->av_packet_unref(pkt);
            continue;
        }
        ret = F->avcodec_send_packet(ctx->audio_dec, pkt);
        F->av_packet_unref(pkt);
        if (ret < 0 && ret != AVERROR(EAGAIN)) {
            grok_av_fail(ctx, GROK_AV_ERR_DECODE, ret);
            status = GROK_AV_ERR_DECODE;
            break;
        }
        for (;;) {
            if (grok_av_is_cancelled(ctx)) {
                status = GROK_AV_ERR_CANCELLED;
                break;
            }
            ret = F->avcodec_receive_frame(ctx->audio_dec, frame);
            if (ret == AVERROR(EAGAIN))
                break;
            if (ret < 0) {
                if (ret == AVERROR_EOF)
                    break;
                grok_av_fail(ctx, GROK_AV_ERR_DECODE, ret);
                status = GROK_AV_ERR_DECODE;
                break;
            }
            int in_count = frame->nb_samples;
            int out_bound = F->swr_get_out_samples(ctx->swr, in_count);
            if (out_bound < 0)
                out_bound = in_count + 1024;
            uint64_t remaining =
                max_samples == UINT64_MAX ? UINT64_MAX
                                          : max_samples - total_frames;
            int out_count = remaining == UINT64_MAX
                                ? out_bound
                                : (remaining < (uint64_t)out_bound
                                       ? (int)remaining
                                       : out_bound);
            if (out_count > 0) {
                size_t needed =
                    used + (size_t)out_count * (size_t)channels *
                               bytes_per_sample;
                if (needed > capacity) {
                    uint8_t *nb = grok_av_buf_alloc(F, needed);
                    if (nb == NULL) {
                        status = GROK_AV_ERR_NOMEM;
                        break;
                    }
                    memcpy(nb, buf, used);
                    grok_av_buf_free(buf);
                    buf = nb;
                    capacity = needed;
                }
                uint8_t *write_ptr = buf + used;
                int converted = F->swr_convert(
                    ctx->swr, &write_ptr, out_count,
                    (const uint8_t **)frame->extended_data, in_count);
                if (converted < 0) {
                    grok_av_fail(ctx, GROK_AV_ERR_DECODE, converted);
                    status = GROK_AV_ERR_DECODE;
                    break;
                }
                used += (size_t)converted * (size_t)channels *
                        bytes_per_sample;
                total_frames += (uint64_t)converted;
            }
            F->av_frame_unref(frame);
            if (max_samples != UINT64_MAX && total_frames >= max_samples) {
                truncated = 1;
                done = 1;
                break;
            }
        }
        if (status != GROK_AV_OK)
            break;
        if (done)
            break;
    }

    F->av_frame_free(&frame);
    F->av_packet_free(&pkt);
    if (status != GROK_AV_OK) {
        grok_av_buf_free(buf);
        return status;
    }

    out->data = (float *)buf;
    out->len = used / bytes_per_sample;
    out->sample_rate = par->sample_rate;
    out->channels = channels;
    out->truncated = truncated;
    return GROK_AV_OK;
}

/* ------------------------------------------------------------------ */
/* Public entry points                                                  */
/* ------------------------------------------------------------------ */

int grok_av_open(GrokAvContext **out_ctx, const GrokAvFns *fns,
                 const uint8_t *data, size_t len, const GrokAvLimits *limits) {
    if (out_ctx == NULL || fns == NULL || data == NULL || len == 0 ||
        limits == NULL)
        return GROK_AV_ERR_INVALID_ARG;
    if (limits->max_source_bytes > 0 && len > limits->max_source_bytes)
        return GROK_AV_ERR_LIMIT;

    GrokAvContext *ctx = (GrokAvContext *)calloc(1, sizeof(*ctx));
    if (ctx == NULL)
        return GROK_AV_ERR_NOMEM;
    ctx->fns = *fns;
    ctx->limits = *limits;
    ctx->data = data;
    ctx->len = len;
    ctx->pos = 0;
    ctx->video_stream_index = -1;
    ctx->audio_stream_index = -1;
    atomic_init(&ctx->cancel_flag, 0);
    snprintf(ctx->errbuf, sizeof(ctx->errbuf), "no error");

    const GrokAvFns *F = &ctx->fns;

    ctx->avio_buf = (uint8_t *)F->av_malloc(1u << 15);
    if (ctx->avio_buf == NULL) {
        *out_ctx = ctx;
        return GROK_AV_ERR_NOMEM;
    }
    ctx->avio = F->avio_alloc_context(ctx->avio_buf, 1u << 15, 0, ctx,
                                      grok_av_io_read, NULL, grok_av_io_seek);
    if (ctx->avio == NULL) {
        *out_ctx = ctx;
        return GROK_AV_ERR_NOMEM;
    }
    ctx->fmt = F->avformat_alloc_context();
    if (ctx->fmt == NULL) {
        *out_ctx = ctx;
        return GROK_AV_ERR_NOMEM;
    }
    ctx->fmt->pb = ctx->avio;
    ctx->fmt->flags |= AVFMT_FLAG_CUSTOM_IO;
    ctx->fmt->interrupt_callback.callback = grok_av_interrupt;
    ctx->fmt->interrupt_callback.opaque = ctx;

    int ret = F->avformat_open_input(&ctx->fmt, NULL, NULL, NULL);
    if (ret < 0) {
        /* avformat_open_input frees *ps and sets it NULL on failure. */
        ctx->fmt = NULL;
        grok_av_fail(ctx, GROK_AV_ERR_OPEN, ret);
        *out_ctx = ctx;
        return GROK_AV_ERR_OPEN;
    }
    ret = F->avformat_find_stream_info(ctx->fmt, NULL);
    if (ret < 0) {
        grok_av_fail(ctx, GROK_AV_ERR_OPEN, ret);
        *out_ctx = ctx;
        return GROK_AV_ERR_OPEN;
    }

    /*
     * Strict duration enforcement: reject media whose probed duration
     * exceeds the cap. The duration is reliable only when FFmpeg reports a
     * non-negative value (`AVFormatContext::duration` is AV_NOPTS_VALUE,
     * i.e. INT64_MIN, when unknown). Unknown or otherwise negative
     * durations pass through: the remaining caps (bytes, pixels,
     * dimensions, frames, PCM samples, request deadline) still bound the
     * session. The comparison is overflow-safe: after the non-negative
     * guard the cast to uint64_t is exact, and a cap above INT64_MAX can
     * never be exceeded by an int64_t duration, so no signed/unsigned
     * comparison is ever performed on a negative value.
     */
    if (ctx->limits.max_duration_us > 0 && ctx->fmt->duration >= 0 &&
        (uint64_t)ctx->fmt->duration > ctx->limits.max_duration_us) {
        snprintf(ctx->errbuf, sizeof(ctx->errbuf),
                 "media duration %" PRId64 " us exceeds cap %" PRIu64 " us",
                 ctx->fmt->duration, ctx->limits.max_duration_us);
        *out_ctx = ctx;
        return GROK_AV_ERR_LIMIT;
    }

    const AVCodec *video_codec = NULL;
    const AVCodec *audio_codec = NULL;
    ctx->video_stream_index =
        F->av_find_best_stream(ctx->fmt, AVMEDIA_TYPE_VIDEO, -1, -1,
                               &video_codec, 0);
    ctx->audio_stream_index =
        F->av_find_best_stream(ctx->fmt, AVMEDIA_TYPE_AUDIO, -1, -1,
                               &audio_codec, 0);
    if (ctx->video_stream_index >= 0)
        ctx->video_codec = video_codec;
    if (ctx->audio_stream_index >= 0)
        ctx->audio_codec = audio_codec;

    *out_ctx = ctx;
    return GROK_AV_OK;
}

int grok_av_probe(GrokAvContext *ctx, GrokAvProbeResult *out) {
    if (ctx == NULL || out == NULL)
        return GROK_AV_ERR_INVALID_ARG;
    memset(out, 0, sizeof(*out));
    out->video_stream_index = -1;
    out->audio_stream_index = -1;
    if (ctx->fmt == NULL) {
        grok_av_fail(ctx, GROK_AV_ERR_OPEN, 0);
        return GROK_AV_ERR_OPEN;
    }
    if (ctx->video_stream_index >= 0) {
        AVStream *st = ctx->fmt->streams[ctx->video_stream_index];
        AVCodecParameters *par = st->codecpar;
        out->has_video = 1;
        out->width = par->width;
        out->height = par->height;
        out->video_stream_index = ctx->video_stream_index;
        out->video_time_base_num = st->time_base.num;
        out->video_time_base_den = st->time_base.den;
    }
    if (ctx->audio_stream_index >= 0) {
        AVStream *st = ctx->fmt->streams[ctx->audio_stream_index];
        AVCodecParameters *par = st->codecpar;
        out->has_audio = 1;
        out->audio_stream_index = ctx->audio_stream_index;
        out->sample_rate = par->sample_rate;
        out->channels = par->ch_layout.nb_channels;
    }
    out->duration_us = ctx->fmt->duration;
    out->start_time_us = ctx->fmt->start_time;
    return GROK_AV_OK;
}

int grok_av_next_frame(GrokAvContext *ctx, GrokAvFrame *out) {
    if (ctx == NULL || out == NULL)
        return GROK_AV_ERR_INVALID_ARG;
    memset(out, 0, sizeof(*out));
    return grok_av_decode_video(ctx, NULL, out);
}

int grok_av_frame_at_seconds(GrokAvContext *ctx, int64_t seconds,
                             GrokAvFrame *out) {
    const GrokAvFns *F = &ctx->fns;
    if (ctx == NULL || out == NULL)
        return GROK_AV_ERR_INVALID_ARG;
    memset(out, 0, sizeof(*out));
    if (seconds < 0)
        return GROK_AV_ERR_INVALID_ARG;
    if (ctx->video_stream_index < 0)
        return grok_av_no_stream(ctx, "video");
    int ret = grok_av_prepare_video_decoder(ctx);
    if (ret != GROK_AV_OK)
        return ret;

    AVStream *st = ctx->fmt->streams[ctx->video_stream_index];
    if (st->time_base.num == 0 || st->time_base.den == 0) {
        grok_av_fail(ctx, GROK_AV_ERR_UNSUPPORTED, 0);
        return GROK_AV_ERR_UNSUPPORTED;
    }
    AVRational seconds_tb = {1, 1};
    int64_t target_ts = F->av_rescale_q(seconds, seconds_tb, st->time_base);
    ret = F->avformat_seek_file(ctx->fmt, ctx->video_stream_index, INT64_MIN,
                                target_ts, target_ts, AVSEEK_FLAG_BACKWARD);
    if (ret < 0) {
        grok_av_fail(ctx, GROK_AV_ERR_SEEK, ret);
        return GROK_AV_ERR_SEEK;
    }
    F->avcodec_flush_buffers(ctx->video_dec);

    int64_t min_pts = target_ts;
    return grok_av_decode_video(ctx, &min_pts, out);
}

int grok_av_cancel(GrokAvContext *ctx) {
    if (ctx == NULL)
        return GROK_AV_ERR_INVALID_ARG;
    atomic_store_explicit(&ctx->cancel_flag, 1, memory_order_relaxed);
    return GROK_AV_OK;
}

const char *grok_av_last_error(GrokAvContext *ctx) {
    if (ctx == NULL)
        return "null context";
    return ctx->errbuf;
}

void grok_av_frame_free(GrokAvFrame *frame) {
    if (frame == NULL)
        return;
    grok_av_buf_free(frame->data);
    frame->data = NULL;
}

void grok_av_pcm_free(GrokAvPcm *pcm) {
    if (pcm == NULL)
        return;
    grok_av_buf_free((uint8_t *)pcm->data);
    pcm->data = NULL;
}

void grok_av_close(GrokAvContext *ctx) {
    if (ctx == NULL)
        return;
    const GrokAvFns *F = &ctx->fns;
    if (ctx->sws != NULL)
        F->sws_freeContext(ctx->sws);
    if (ctx->swr != NULL)
        F->swr_free(&ctx->swr);
    if (ctx->rgb_frame != NULL)
        F->av_frame_free(&ctx->rgb_frame);
    if (ctx->video_dec != NULL)
        F->avcodec_free_context(&ctx->video_dec);
    if (ctx->audio_dec != NULL)
        F->avcodec_free_context(&ctx->audio_dec);
    if (ctx->fmt != NULL) {
        ctx->fmt->pb = NULL; /* we own the AVIO context */
        F->avformat_free_context(ctx->fmt);
    }
    if (ctx->avio != NULL)
        F->avio_context_free(&ctx->avio);
    else if (ctx->avio_buf != NULL)
        F->av_free(ctx->avio_buf);
    free(ctx);
}
