import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

// Keeps the typed TypeScript wrappers in sync with the Rust command surface (crates/app/src/rpc.rs).
describe("typed IPC surface", () => {
  const rust = readFileSync(resolve(__dirname, "../../../../crates/app/src/rpc.rs"), "utf8");
  const ts = readFileSync(resolve(__dirname, "commands.ts"), "utf8");
  const block = /pub const COMMANDS: &\[&str\] = &\[([\s\S]*?)\];/.exec(rust)?.[1] ?? "";
  const commands = [...block.matchAll(/"([a-z_]+)"/g)].map((m) => m[1]!);

  it("finds the command list", () => {
    expect(commands.length).toBeGreaterThan(20);
  });

  it.each(commands)("has a typed wrapper for `%s`", (cmd) => {
    expect(ts).toContain(`"${cmd}"`);
  });

  it("wrappers only call commands the backend knows", () => {
    const used = [...ts.matchAll(/call<[^>]*>\("([a-z_]+)"/g)].map((m) => m[1]!);
    for (const c of used) expect(commands).toContain(c);
  });
});
