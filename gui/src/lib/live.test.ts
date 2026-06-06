import { describe, it, expect, vi, afterEach } from "vitest";
import { live, toggleLive, initLivePolling } from "./store.svelte";

describe("live polling", () => {
  afterEach(() => {
    vi.useRealTimers();
    live.paused = false;
  });

  it("toggles paused", () => {
    expect(live.paused).toBe(false);
    toggleLive();
    expect(live.paused).toBe(true);
    toggleLive();
    expect(live.paused).toBe(false);
  });

  it("increments tick on the interval and stops on cleanup; pause halts it", () => {
    vi.useFakeTimers();
    const start = live.tick;
    const stop = initLivePolling(1000);
    vi.advanceTimersByTime(2500);
    expect(live.tick).toBe(start + 2);

    live.paused = true;
    vi.advanceTimersByTime(3000);
    expect(live.tick).toBe(start + 2); // paused — no ticks
    live.paused = false;

    stop();
    vi.advanceTimersByTime(3000);
    expect(live.tick).toBe(start + 2); // stopped — no further ticks
  });
});
