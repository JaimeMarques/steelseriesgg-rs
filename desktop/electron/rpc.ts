import type { Readable, Writable } from "node:stream";
import { mutationTimeout } from "../src/shared/contracts";
type Pending = {
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
  timer: ReturnType<typeof setTimeout>;
};
export class JsonLineClient {
  private sequence = 0;
  private pending = new Map<number, Pending>();
  private buffer = "";
  private closed = false;
  constructor(
    private input: Writable,
    output: Readable,
    private timeoutMs = 5000,
  ) {
    output.setEncoding("utf8");
    output.on("data", (chunk) => this.receive(String(chunk)));
    output.on("end", () => this.close("Audio service disconnected. Reopen SSGG to reconnect."));
    output.on("error", () => this.close("Audio service transport failed."));
    input.on("error", () => this.close("Audio service transport failed."));
  }
  request(method: string, params: unknown): Promise<unknown> {
    if (this.closed) return Promise.reject(new Error("Audio service is disconnected."));
    if (this.pending.size >= 64) return Promise.reject(new Error("Audio service is busy. Try again."));
    const id = ++this.sequence;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(["state.get", "streams.list", "settings.get", "profiles.list", "devices.list"].includes(method)
          ? "Audio service read timed out. Refresh to retry."
          : mutationTimeout));
      }, this.timeoutMs);
      this.pending.set(id, { resolve, reject, timer });
      this.input.write(JSON.stringify({ id, method, params }) + "\n");
    });
  }
  private receive(chunk: string) {
    this.buffer += chunk;
    if (Buffer.byteLength(this.buffer) > 2 * 1024 * 1024) {
      this.close("Audio service response exceeded its limit.");
      return;
    }
    let newline: number;
    while ((newline = this.buffer.indexOf("\n")) >= 0) {
      const line = this.buffer.slice(0, newline);
      this.buffer = this.buffer.slice(newline + 1);
      if (!line.trim()) continue;
      try {
        const response = JSON.parse(line);
        if (!response || !Number.isSafeInteger(response.id)) throw new Error("Malformed response");
        const pending = this.pending.get(response.id);
        if (!pending) continue;
        clearTimeout(pending.timer);
        this.pending.delete(response.id);
        if (response.error)
          pending.reject(
            new Error(
              typeof response.error.message === "string"
                ? response.error.message.slice(0, 1024)
                : "Audio service rejected the change.",
            ),
          );
        else if (Object.hasOwn(response, "result")) pending.resolve(response.result);
        else pending.reject(new Error("Audio service returned an invalid response."));
      } catch {
        this.close("Audio service returned invalid JSON.");
        return;
      }
    }
  }
  close(message = "Audio service disconnected.") {
    this.closed = true;
    for (const entry of this.pending.values()) {
      clearTimeout(entry.timer);
      entry.reject(new Error(message));
    }
    this.pending.clear();
    this.buffer = "";
  }
}
