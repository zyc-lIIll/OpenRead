import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const packageRoot = resolve(root, "node_modules/pdfjs-dist");
const publicRoot = resolve(root, "node_modules/.cache/openread-public/pdfjs");
await rm(publicRoot, { recursive: true, force: true });
await mkdir(publicRoot, { recursive: true });
for (const folder of ["cmaps", "iccs", "standard_fonts", "wasm"]) {
  await cp(resolve(packageRoot, folder), resolve(publicRoot, folder), { recursive: true });
}
await writeFile(resolve(publicRoot, "LICENSE"), await readFile(resolve(packageRoot, "LICENSE")));
