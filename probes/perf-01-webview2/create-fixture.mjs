// Deterministic public-only fixture. No provider, account or user chat input.
import {createHash} from "node:crypto";
import {mkdir, realpath, writeFile} from "node:fs/promises";
import {dirname, win32} from "node:path";
import {fileURLToPath} from "node:url";

export function generateFixture(turns = 3000) {
  if (!Number.isSafeInteger(turns) || turns < 1 || turns > 3000) {
    throw Error("Invalid synthetic turn count");
  }
  const messages = [];
  for (let n = 1; n <= turns; n++) {
    for (const role of ["user", "assistant"]) {
      const suffix = String(n).padStart(5, "0");
      messages.push({
        kind: role + "_message",
        messageId: "synthetic-" + role + "-" + suffix,
        turnId: "synthetic-turn-" + suffix,
        text: "PERF01 public synthetic " + suffix + " " + role + " marker-perf01 " + "x".repeat(240),
        time: 1792830000000 + n * 1000 + Number(role === "assistant")
      });
    }
  }
  return messages;
}

export async function createFixture(outputPath) {
  if (process.platform !== "win32") throw Error("Fixture generation requires approved Windows D: host");
  const resolved = win32.resolve(outputPath);
  if (!resolved.toLowerCase().startsWith("d:\\projects\\cos-rust\\") ||
      !resolved.toLowerCase().endsWith(".json")) {
    throw Error("Fixture output must be a D: project .json path");
  }
  await mkdir(dirname(resolved), {recursive:true});
  const realParent = await realpath(dirname(resolved));
  if (!win32.normalize(realParent).toLowerCase().startsWith("d:\\projects\\cos-rust\\")) {
    throw Error("Real output parent escapes D: project root");
  }
  const content = Buffer.from(JSON.stringify(generateFixture()), "utf8");
  await writeFile(resolved, content, {flag:"wx"});
  return {path:resolved, messages:6000, sha256:createHash("sha256").update(content).digest("hex")};
}

if (process.argv[1] && fileURLToPath(import.meta.url) === win32.resolve(process.argv[1])) {
  createFixture(process.argv[2] ?? "").then(
    receipt => process.stdout.write(JSON.stringify(receipt) + "\n"),
    error => {process.stderr.write(error.message + "\n");process.exitCode = 1;}
  );
}
