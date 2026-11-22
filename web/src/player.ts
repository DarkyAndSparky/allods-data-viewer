import { h } from "./dom";
import type { VideoInfo } from "./api";
import { yuvToRgba } from "./yuv";

/** What the player needs from the app: the two video calls and the translated labels. */
export interface PlayerDeps {
  open(): Promise<VideoInfo>;
  frames(start: number, count: number): Promise<ArrayBuffer>;
  /** The soundtrack as a WAV file; an empty buffer when the video has no sound. */
  audio(): Promise<ArrayBuffer>;
  /** Called when playback starts (the app pauses its own sound player). */
  onPlay(): void;
  /** Translates one of the `video*` keys and `loop`. */
  label(key: "videoPlay" | "videoPause" | "videoBuffering" | "videoPosition" | "videoFailed" | "loop"): string;
}

export interface Player {
  el: HTMLElement;
  dispose(): void;
}

/** Frames fetched per call, and how many frames ahead of the picture are kept ready. */
const BATCH = 16;
const AHEAD = 32;

function clock(seconds: number): string {
  const s = Math.max(0, seconds);
  const m = Math.floor(s / 60);
  return `${m}:${(s - m * 60).toFixed(1).padStart(4, "0")}`;
}

/**
 * A video player on a canvas: frames come from the app already decoded (planar YCbCr), are
 * converted here and shown at the frame rate of the file. A spinner shows while frames load.
 */
export function createPlayer(deps: PlayerDeps): Player {
  const canvas = h("canvas", { class: "video-canvas" });
  const spinner = h("span", { class: "spinner", attrs: { role: "status", hidden: "" } });
  const playBtn = h("button", { class: "btn", attrs: { type: "button", disabled: "" } });
  const slider = h("input", { attrs: { type: "range", min: "0", max: "0", value: "0", step: "1", disabled: "" } }) as HTMLInputElement;
  const time = h("span", { class: "muted video-time" });
  const loopBox = h("input", { attrs: { type: "checkbox" } }) as HTMLInputElement;
  const loopLabel = h("label", { class: "loop" }, loopBox, h("span", { text: deps.label("loop") }));
  const sound = new Audio();
  sound.preload = "auto";
  let soundUrl = "";
  let hasSound = false;
  const message = h("p", { class: "muted", attrs: { hidden: "" } });
  const stage = h("div", { class: "video-stage" }, canvas, spinner);
  const controls = h("div", { class: "video-controls" }, playBtn, slider, time, loopLabel);
  const el = h("div", { class: "video" }, stage, controls, message);

  const cache = new Map<number, Uint8Array>();
  let info: VideoInfo | null = null;
  let image: ImageData | null = null;
  let pos = 0;
  let shown = -1;
  let playing = false;
  let stalled = false;
  let fetching: Promise<void> | null = null;
  let raf = 0;
  let clockStart = 0;
  let clockPos = 0;
  let disposed = false;

  const busy = (on: boolean): void => {
    if (on) spinner.removeAttribute("hidden");
    else spinner.setAttribute("hidden", "");
    spinner.setAttribute("aria-label", deps.label("videoBuffering"));
  };

  const fail = (e: unknown): void => {
    playing = false;
    sound.pause();
    busy(false);
    const detail = typeof e === "object" && e !== null && "detail" in e ? String((e as { detail: unknown }).detail) : String(e);
    message.textContent = `${deps.label("videoFailed")} ${detail}`;
    message.removeAttribute("hidden");
    refreshButton();
  };

  const refreshButton = (): void => {
    playBtn.textContent = playing ? deps.label("videoPause") : deps.label("videoPlay");
  };

  const draw = (i: number): boolean => {
    const frame = cache.get(i);
    if (!frame || !info || !image) return false;
    yuvToRgba(frame, info, image.data);
    canvas.getContext("2d")?.putImageData(image, 0, 0);
    shown = i;
    slider.value = String(i);
    time.textContent = deps.label("videoPosition")
      .replace("{{at}}", clock(i / info.fps))
      .replace("{{total}}", clock(info.frames / info.fps));
    return true;
  };

  const firstMissing = (from: number): number => {
    let i = from;
    while (info && i < info.frames && cache.has(i)) i += 1;
    return i;
  };

  const fetchFrom = (start: number): Promise<void> => {
    if (fetching) return fetching;
    const inf = info;
    if (!inf) return Promise.resolve();
    fetching = deps
      .frames(start, BATCH)
      .then((buf) => {
        if (disposed) return;
        const bytes = new Uint8Array(buf);
        const n = Math.floor(bytes.length / inf.frame_bytes);
        for (let k = 0; k < n; k++) {
          cache.set(start + k, bytes.subarray(k * inf.frame_bytes, (k + 1) * inf.frame_bytes));
        }
      })
      .catch(fail)
      .finally(() => {
        fetching = null;
      });
    return fetching;
  };

  /** Shows frame `i`, loading it first when needed. */
  const show = async (i: number): Promise<void> => {
    if (!info) return;
    pos = Math.min(Math.max(0, i), info.frames - 1);
    if (!cache.has(pos)) {
      busy(true);
      while (!disposed && !cache.has(pos) && message.hasAttribute("hidden")) {
        const before = cache.size;
        await fetchFrom(pos);
        if (cache.size === before) break;
      }
      busy(false);
    }
    if (!disposed) draw(pos);
  };

  const tick = (now: number): void => {
    raf = 0;
    if (!playing || disposed || !info) return;
    if (stalled) {
      raf = requestAnimationFrame(tick);
      return;
    }
    // The frame timestamp can be a little older than `clockStart`; never go below the start.
    // The sound is the clock while it plays; after its end (or without sound) the wall clock
    // that started with Play takes over.
    const onSound = hasSound && !sound.paused && !sound.ended;
    const target = onSound
      ? Math.floor(sound.currentTime * info.fps)
      : clockPos + Math.max(0, Math.floor(((now - clockStart) * info.fps) / 1000));
    if (target >= info.frames) {
      if (loopBox.checked) {
        pos = 0;
        void show(0).then(() => {
          playing = false;
          play();
        });
        return;
      }
      playing = false;
      sound.pause();
      refreshButton();
      return;
    }
    if (cache.has(target)) {
      if (target !== shown) draw(target);
      pos = target;
      for (const k of cache.keys()) if (k < target - 2) cache.delete(k);
      const missing = firstMissing(target);
      if (missing < Math.min(info.frames, target + AHEAD)) void fetchFrom(missing);
    } else {
      stalled = true;
      busy(true);
      sound.pause();
      void fetchFrom(target).then(() => {
        stalled = false;
        busy(false);
        clockPos = target;
        clockStart = performance.now();
        if (hasSound && playing) {
          sound.currentTime = target / (info?.fps ?? 1);
          void sound.play().catch(() => undefined);
        }
      });
    }
    raf = requestAnimationFrame(tick);
  };

  const play = (): void => {
    if (!info || playing) return;
    if (pos >= info.frames - 1) pos = 0;
    playing = true;
    deps.onPlay();
    clockPos = pos;
    clockStart = performance.now();
    if (hasSound) {
      sound.currentTime = pos / info.fps;
      void sound.play().catch(() => undefined);
    }
    refreshButton();
    raf = requestAnimationFrame(tick);
  };

  const pause = (): void => {
    playing = false;
    sound.pause();
    refreshButton();
  };

  playBtn.addEventListener("click", () => (playing ? pause() : play()));
  slider.addEventListener("input", () => {
    const wasPlaying = playing;
    pause();
    void show(Number(slider.value)).then(() => {
      if (wasPlaying) play();
    });
  });

  busy(true);
  refreshButton();
  // The soundtrack loads next to the picture; a video without sound or with damaged sound
  // still plays silently.
  deps
    .audio()
    .then((wav) => {
      if (disposed || wav.byteLength === 0) return;
      soundUrl = URL.createObjectURL(new Blob([wav], { type: "audio/wav" }));
      sound.src = soundUrl;
      hasSound = true;
    })
    .catch(() => undefined);
  deps
    .open()
    .then(async (v) => {
      if (disposed) return;
      info = v;
      canvas.width = v.width;
      canvas.height = v.height;
      image = new ImageData(v.width, v.height);
      slider.max = String(Math.max(0, v.frames - 1));
      slider.removeAttribute("disabled");
      playBtn.removeAttribute("disabled");
      await show(0);
    })
    .catch(fail);

  return {
    el,
    dispose(): void {
      disposed = true;
      playing = false;
      sound.pause();
      sound.removeAttribute("src");
      if (soundUrl) URL.revokeObjectURL(soundUrl);
      if (raf) cancelAnimationFrame(raf);
      cache.clear();
    },
  };
}
