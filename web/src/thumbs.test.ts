import { describe, expect, it } from "vitest";
import { JobQueue } from "./thumbs";

describe("JobQueue", () => {
  it("never runs more jobs than the limit and keeps the order", async () => {
    const q = new JobQueue(2);
    let running = 0;
    let peak = 0;
    const order: number[] = [];
    const job = (n: number) => async () => {
      running++;
      peak = Math.max(peak, running);
      await new Promise((r) => setTimeout(r, 5));
      order.push(n);
      running--;
      return n;
    };
    const out = await Promise.all([1, 2, 3, 4, 5].map((n) => q.add(job(n))));
    expect(out).toEqual([1, 2, 3, 4, 5]);
    expect(peak).toBe(2);
    expect(order).toHaveLength(5);
  });

  it("clear drops jobs that have not started", async () => {
    const q = new JobQueue(1);
    const ran: number[] = [];
    const a = q.add(async () => {
      await new Promise((r) => setTimeout(r, 5));
      ran.push(1);
    });
    void q.add(async () => ran.push(2));
    q.clear();
    await a;
    await new Promise((r) => setTimeout(r, 15));
    expect(ran).toEqual([1]);
  });
});
