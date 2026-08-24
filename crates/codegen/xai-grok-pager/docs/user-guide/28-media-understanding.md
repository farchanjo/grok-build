# Media Understanding

Grok can preserve useful context from images, audio files, and videos even when the active conversation model cannot consume that media type directly. The shell converts unsupported media into bounded text descriptions or transcripts, stores those descriptors with the session, and reuses them during later compaction.

This feature extends normal attachments and `read_file`; it does not add a separate conversation mode.

---

## Routing Modes

Configure media routing in `config.toml`:

```toml
[media]
mode = "auto"                 # auto | tools_only | off
image_model = "@session"      # session route or catalog model ID
audio_model = "xai-streaming-stt" # optional; xAI streaming STT aliases only
video_model = ""                   # optional frame-description catalog model ID
file_model = ""                    # optional file/PDF catalog model ID; unset reuses image_model
image_limit = 16               # 1..=64
audio_max_seconds = 120        # 1..=900
video_max_seconds = 600        # 1..=7200
video_max_frames = 8           # 1..=32
```

You only need to include values that differ from the defaults.

| Mode | Behavior |
|------|----------|
| `auto` (default) | Uses native model input when support is explicitly advertised. Otherwise, Grok may invoke an auxiliary media route and may lazily backfill missing image descriptors before host-owned compaction. |
| `tools_only` | Auxiliary media understanding runs only for explicit tool results such as `read_file`; automatic attachment conversion and compaction backfill are disabled. Existing descriptors remain available to compaction. |
| `off` | Disables auxiliary media inference. Media the active model supports natively may still be sent through that native route; unsupported media is represented by an explicit failure or placeholder rather than being silently dropped. Existing stored descriptors remain available to compaction. |

You can edit the routing mode and model routes from `/settings` under **Models**. Changes are written to `[media]` in `config.toml`.

---

## Model Capabilities

Each catalog model may advertise these tri-state fields:

```toml
[model.my-model]
supports_image_input = true
supports_audio_input = false
supports_video_input = false
```

Each field has three states:

- `true`: native input is explicitly supported;
- `false`: native input is explicitly unsupported;
- omitted: support is unknown.

Unknown is intentionally not treated as supported. This prevents Grok from sending media in a request shape that an unverified endpoint may reject. For custom models, declare only capabilities you have confirmed with that endpoint.

Remote model catalogs can also supply modality metadata. A local `[model.<id>]` override wins over inherited or discovered metadata.

---

## Images

When the active model explicitly supports image input, Grok keeps the normal native image path. Otherwise, in `auto` mode, the configured image route produces a text description before the active model sees the turn. The same vision fallback covers user attachments, `read_file` images, PDF page renders, sampled video frames, and images extracted from other tool results.

`image_model = "@session"` reuses the active route when that model advertises `supports_image_input = true`. If the session model is text-only or unknown, Grok automatically selects a catalog model that advertises image input: same provider first, then first-party xAI, then any other credentialed vision model. Pin `[media].image_model`, `[media].video_model`, or `[media].file_model` in `/settings` under **Models** when you want a specific route. If no vision model is in the catalog, the turn or tool result fails closed with an explicit diagnostic.

Image descriptions are cached by media content, source, and prompt fingerprint. Tool reads and compaction backfill use a stable prompt so their descriptors can be reused across turns.

---

## Audio

`read_file` recognizes common audio formats before generic binary-file rejection. Audio extraction is bounded by `audio_max_seconds`; media metadata and any available transcript are converted to text. Raw audio is not added as a persistent conversation content variant.

File transcription reuses the native xAI streaming STT transport without microphone capture. Supported `audio_model` values are empty, `@session`, `xai`, `xai-stt`, and `xai-streaming-stt`; all resolve to that same xAI route. Arbitrary catalog or third-party audio routes fail closed with an explicit diagnostic. Authentication must be available from the current xAI session or API-key provider. File STT uses the same `[voice].api_base` → `[endpoints].xai_api_base_url` endpoint precedence as microphone dictation, including enterprise endpoint overrides.

If transcription is unavailable or extraction fails, the model receives an explicit diagnostic instead of silently losing the attachment. Audio and video are currently normalized to text even if a catalog advertises native audio or video input; native attachment pass-through is implemented only for images.

---

## Video

`read_file` recognizes common video formats and probes their metadata. Video understanding samples at most `video_max_frames` frames from at most `video_max_seconds` of the file. Frames are described through `video_model` when configured, otherwise through `image_model`. This is image-based frame understanding rather than a raw-video inference request. An available audio track is bounded by the audio limit and uses xAI streaming STT.

Frame extraction uses argv-only `ffmpeg`/`ffprobe` subprocesses with input-size, captured-output, and wall-clock limits. Grok reports missing tools or extraction failures explicitly.

---

## Files

`read_file` is the coding tool. Source, JSON, Markdown, and any other **text** payload (`FileContent`) always stay on the **session** model. The File model is never used to rewrite or re-sample that text.

The File model is only for **binary** payloads the session model cannot consume natively:

- PDF pages rendered as images (`PdfPageImages`);
- image files (`ImageContent`);
- video frames (via the Video model, falling back to Image).

The file route is `[media].file_model` when set in `/settings` (File model). When that setting is Unset, Grok reuses the image understanding model, then `@session`, then the same catalog vision fallback used for images. Office documents that `read_file` extracts as text follow the session path, not this auxiliary route.

---

## Persistence and Compaction

Text descriptors are stored in the session directory:

```text
$GROK_HOME/sessions/<encoded-cwd>/<session-id>/media_descriptors.jsonl
```

The descriptor store is session-scoped. New records append efficiently, while replacements and size pressure trigger an owner-only atomic compaction so the JSONL file remains bounded. Records contain scrubbed text plus confined metadata; they do not contain raw image, audio, or video bytes. Asset paths must be relative and cannot escape the session directory.

Before host-owned compaction, Grok freezes one immutable descriptor snapshot and reuses it for all retries, fallback routes, rolling chunks, recap, two-pass compaction, and memory flushes in that attempt. This keeps media context stable and prevents a retry from invoking media understanding again with different output.

External-agent runtimes own their own conversation and do not receive lazy host-side compaction backfill.

---

## Privacy

An auxiliary media model receives the media content needed to create its description or transcript. If that route belongs to an external provider, that provider's retention and privacy policy applies.

The `/settings` media status marks catalog routes recognized as external providers. Before selecting one, consider whether the media may contain source code, screenshots, customer information, recordings, credentials, or other sensitive data.

Grok stores only the resulting scrubbed descriptor text and confined metadata in `media_descriptors.jsonl`. Provider request payloads and raw audio/video bytes are not written there.

---

## Migrating from Image Description Settings

The older image-only setting remains compatible:

```toml
[models]
image_description = "my-vision-model"
```

The `GROK_IMAGE_DESCRIPTION_MODEL` environment variable also remains supported. When `[media].image_model` is absent, resolution uses the legacy environment/config/remote image-description value and then falls back to `@session`.

To migrate explicitly:

```toml
[media]
image_model = "my-vision-model"
```

After confirming the new setting works, you may remove `[models].image_description`. Do not configure both unless you intentionally want `[media].image_model` to take precedence.

---

## Troubleshooting

### The active model rejects an image

- Confirm the model advertises `supports_image_input = true` only if the endpoint really accepts images.
- For a text-only session model, Grok auto-selects a catalog vision route when `[media].image_model` is `@session`. Pin a specific vision model if you want to override that choice, or if the catalog has no image-capable model.

### Audio or video extraction fails

- Install `ffmpeg` and `ffprobe` so both are available on `PATH`.
- Confirm the target is a regular file and within the supported size and duration limits.
- Check the model-facing diagnostic for timeout, malformed media, unsupported codec, or missing-tool details.

### Media is not automatically described

- Verify `[media].mode = "auto"`.
- `tools_only` limits auxiliary processing to explicit tool reads.
- `off` disables auxiliary inference.
- Confirm the selected auxiliary model exists in the current catalog and has suitable input capabilities.
- For `@session` on a text-only model, confirm at least one catalog entry has `supports_image_input = true` and usable credentials.

### Compaction shows a placeholder

The original turn may predate descriptor persistence, media backfill may be disabled, the configured route may be unavailable, or the bounded image limit may have been reached. Grok preserves an explicit placeholder rather than leaking raw media into a text-only compaction request.

---

## See Also

- [Configuration](05-configuration.md) — Complete `config.toml` overview
- [Custom Models](11-custom-models.md) — Model capability metadata and provider setup
- [Compaction Settings](25-compaction.md) — Compaction routes, privacy, and durability
- [Session Management](17-sessions.md) — Session persistence and replay
