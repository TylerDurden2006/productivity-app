import { spawnSync } from "node:child_process";

const npmGate = process.env.npm_execpath
  ? {
      command: process.execPath,
      args: [process.env.npm_execpath, "run", "build"],
      shell: false,
    }
  : {
      command: "npm",
      args: ["run", "build"],
      shell: process.platform === "win32",
    };

const gates = [
  { command: "cargo", args: ["fmt", "--all", "--", "--check"], shell: false },
  {
    command: "cargo",
    args: ["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
    shell: false,
  },
  { command: "cargo", args: ["test", "--workspace"], shell: false },
  npmGate,
];

for (const gate of gates) {
  process.stdout.write(`\n> ${gate.command} ${gate.args.join(" ")}\n`);
  const result = spawnSync(gate.command, gate.args, {
    stdio: "inherit",
    shell: gate.shell,
  });
  if (result.error) {
    console.error(result.error.message);
    process.exit(1);
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

console.log("\nAll gates passed.");
