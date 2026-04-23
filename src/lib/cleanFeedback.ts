type ToneStep = {
  frequency: number;
  durationMs: number;
  gain: number;
  type?: OscillatorType;
};

/** 从 localStorage 读取音效开关，默认开启 */
function isSoundEnabled(): boolean {
  try {
    const raw = localStorage.getItem("macflow.prefs.v1");
    if (!raw) return true;
    const p = JSON.parse(raw);
    return p?.cleanupSoundEnabled !== false;
  } catch {
    return true;
  }
}

let sharedAudioContext: AudioContext | null = null;

function getAudioContext(): AudioContext | null {
  if (typeof window === "undefined") return null;
  const Ctx = window.AudioContext || (window as typeof window & { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
  if (!Ctx) return null;
  if (!sharedAudioContext) {
    sharedAudioContext = new Ctx();
  }
  return sharedAudioContext;
}

async function playSequence(steps: ToneStep[]) {
  if (!isSoundEnabled()) return;
  const ctx = getAudioContext();
  if (!ctx) return;
  if (ctx.state === "suspended") {
    await ctx.resume();
  }

  const startAt = ctx.currentTime + 0.01;
  let cursor = startAt;

  for (const step of steps) {
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.type = step.type ?? "sine";
    osc.frequency.setValueAtTime(step.frequency, cursor);
    gain.gain.setValueAtTime(0.0001, cursor);
    gain.gain.exponentialRampToValueAtTime(step.gain, cursor + 0.02);
    gain.gain.exponentialRampToValueAtTime(0.0001, cursor + step.durationMs / 1000);
    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.start(cursor);
    osc.stop(cursor + step.durationMs / 1000 + 0.03);
    cursor += step.durationMs / 1000;
  }
}

export async function playCleanStartSound() {
  return playSequence([
    { frequency: 420, durationMs: 90, gain: 0.02, type: "triangle" },
    { frequency: 560, durationMs: 120, gain: 0.025, type: "triangle" },
  ]);
}

export async function playCleanSuccessSound() {
  return playSequence([
    { frequency: 740, durationMs: 110, gain: 0.03, type: "sine" },
    { frequency: 988, durationMs: 140, gain: 0.035, type: "sine" },
    { frequency: 1320, durationMs: 220, gain: 0.03, type: "triangle" },
  ]);
}

export async function playCleanFailureSound() {
  return playSequence([
    { frequency: 360, durationMs: 140, gain: 0.018, type: "sawtooth" },
    { frequency: 250, durationMs: 180, gain: 0.014, type: "triangle" },
  ]);
}

export function animateNumber(
  from: number,
  to: number,
  durationMs: number,
  onUpdate: (value: number) => void,
) {
  if (typeof window === "undefined") {
    onUpdate(to);
    return () => undefined;
  }

  const reduceMotion = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
  if (reduceMotion || durationMs <= 0) {
    onUpdate(to);
    return () => undefined;
  }

  let frame = 0;
  const startedAt = performance.now();
  const delta = to - from;

  const tick = (now: number) => {
    const progress = Math.min((now - startedAt) / durationMs, 1);
    const eased = 1 - Math.pow(1 - progress, 3);
    onUpdate(Math.round(from + delta * eased));
    if (progress < 1) {
      frame = requestAnimationFrame(tick);
    }
  };

  frame = requestAnimationFrame(tick);
  return () => cancelAnimationFrame(frame);
}
