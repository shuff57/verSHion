// Regenerates src/ai/prompt.ts from the app's conversion prompt. Run from server/worker: node scripts/gen-ai-prompt.mjs
import { readFileSync, writeFileSync } from "node:fs";
const md = readFileSync(new URL("../../../prompts/ditto-convert.md", import.meta.url), "utf8");
writeFileSync(new URL("../src/ai/prompt.ts", import.meta.url), "// GENERATED from prompts/ditto-convert.md by scripts/gen-ai-prompt.mjs. Do not edit: ai.test.ts fails if they differ.\n// The desktop app embeds the same file (src/ai.rs TRANSCRIBE_PROMPT), so a job and a direct page read send one prompt.\nexport const DITTO_PROMPT = " + JSON.stringify(md) + ";\n");
